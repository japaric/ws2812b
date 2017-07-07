#![deny(warnings)]
#![feature(const_fn)]
#![feature(plugin)]
#![feature(used)]
#![no_std]
#![plugin(rtfm_macros)]

extern crate blue_pill;

#[macro_use(task)]
extern crate cortex_m_rtfm as rtfm;

extern crate shared;

use blue_pill::dma::{Buffer, Dma1Channel2, Dma1Channel4, Dma1Channel5};
use blue_pill::prelude::*;
use blue_pill::stm32f103xx::Interrupt;
use blue_pill::time::{Hertz, Microseconds};
use blue_pill::{Channel, Pwm, Serial, Timer};
use rtfm::Threshold;
use shared::State;

// CONFIGURATION
const _0: u8 = 3;
const _1: u8 = 7;
const BAUD_RATE: Hertz = Hertz(115_200);
const LATCH_DELAY: Microseconds = Microseconds(50);
const LOG_FREQUENCY: Hertz = Hertz(1);
const WS2812B_FREQUENCY: Hertz = Hertz(400_000);

// TASKS AND RESOURCES
rtfm! {
    device: blue_pill::stm32f103xx,

    resources: {
        BUSY: bool = false;
        CONTEXT_SWITCHES: u16 = 0;
        FRAMES: u8 = 0;
        RGB_ARRAY: [u8; 72] = [0; 72];
        RX_BUFFER: Buffer<[u8; 72], Dma1Channel5> = Buffer::new([0; 72]);
        SLEEP_CYCLES: u32 = 0;
        TX_BUFFER: Buffer<[u8; 13], Dma1Channel4> = Buffer::new([0; 13]);
        WS2812B_BUFFER: Buffer<[u8; 577], Dma1Channel2> = Buffer::new([0; 577]);
    },

    init: {
        path: init,
    },

    idle: {
        path: idle,
        resources: [
            DWT,
            SLEEP_CYCLES,
        ],
    },

    tasks: {
        DMA1_CHANNEL2: {
            enabled: true,
            priority: 1,
            resources: [
                CONTEXT_SWITCHES,
                DMA1,
                TIM1,
                WS2812B_BUFFER,
            ],
        },

        DMA1_CHANNEL4: {
            enabled: true,
            priority: 1,
            resources: [
                CONTEXT_SWITCHES,
                DMA1,
                TX_BUFFER,
            ],
        },

        DMA1_CHANNEL5: {
            enabled: true,
            priority: 1,
            resources: [
                BUSY,
                CONTEXT_SWITCHES,
                DMA1,
                RGB_ARRAY,
                RX_BUFFER,
                USART1,
            ],
        },

        EXTI0: {
            enabled: true,
            priority: 1,
            resources: [
                CONTEXT_SWITCHES,
                DMA1,
                RGB_ARRAY,
                TIM2,
                WS2812B_BUFFER,
            ],
        },

        TIM1_UP_TIM10: {
            enabled: true,
            priority: 1,
            resources: [
                BUSY,
                CONTEXT_SWITCHES,
                FRAMES,
                TIM1,
            ],
        },

        TIM3: {
            enabled: true,
            priority: 1,
            resources: [
                CONTEXT_SWITCHES,
                DMA1,
                DWT,
                FRAMES,
                SLEEP_CYCLES,
                TIM3,
                TX_BUFFER,
                USART1,
            ],
        },
    },
}

// INITIALIZATION
fn init(p: init::Peripherals, r: init::Resources) {
    let pwm = Pwm(p.TIM2);
    let serial = Serial(p.USART1);
    let timer1 = Timer(p.TIM1);
    let timer3 = Timer(p.TIM3);

    p.DWT.enable_cycle_counter();

    timer1.init(LATCH_DELAY, p.RCC);

    timer3.init(LOG_FREQUENCY.invert(), p.RCC);

    serial.init(BAUD_RATE.invert(), p.AFIO, Some(p.DMA1), p.GPIOA, p.RCC);

    pwm.init(
        WS2812B_FREQUENCY.invert(),
        p.AFIO,
        Some(p.DMA1),
        p.GPIOA,
        p.RCC,
    );
    pwm.enable(Channel::_1);

    serial.read_exact(p.DMA1, r.RX_BUFFER).unwrap();

    timer3.resume();
}

// IDLE LOOP
fn idle(_t: Threshold, mut r: idle::Resources) -> ! {
    loop {
        rtfm::atomic(|cs| {
            let dwt = r.DWT.borrow(cs);
            let sleep_cycles = r.SLEEP_CYCLES.borrow_mut(cs);

            // Sleep
            let before = dwt.cyccnt.read();
            rtfm::wfi();
            let after = dwt.cyccnt.read();

            let elapsed = after.wrapping_sub(before);
            **sleep_cycles += elapsed;
        });

        // interrupts are serviced here
    }
}

// TASKS
task!(TIM3, log);

fn log(_t: Threshold, r: TIM3::Resources) {
    let timer = Timer(r.TIM3);
    let serial = Serial(r.USART1);

    timer.wait().unwrap();

    let snapshot = r.DWT.cyccnt.read();
    let state = State {
        context_switches: **r.CONTEXT_SWITCHES,
        frames: **r.FRAMES,
        sleep_cycles: **r.SLEEP_CYCLES,
        snapshot: snapshot,
    };
    state.serialize(&mut *r.TX_BUFFER.borrow_mut());

    serial.write_all(r.DMA1, r.TX_BUFFER).unwrap();

    **r.CONTEXT_SWITCHES = 0;
    **r.FRAMES = 0;
    **r.SLEEP_CYCLES = 0;
}

task!(DMA1_CHANNEL4, tx_transfer_done);

fn tx_transfer_done(_t: Threshold, r: DMA1_CHANNEL4::Resources) {
    **r.CONTEXT_SWITCHES += 1;

    r.TX_BUFFER.release(r.DMA1).unwrap();
}

task!(DMA1_CHANNEL5, rx);

fn rx(_t: Threshold, r: DMA1_CHANNEL5::Resources) {
    **r.CONTEXT_SWITCHES += 1;

    let serial = Serial(r.USART1);

    r.RX_BUFFER.release(r.DMA1).unwrap();

    // When busy we just ignore incoming frames
    // TODO we can probably double throughput if we turn this into a pipeline
    // where an incoming RGB frame is transformed into a WS2812B frame while a
    // previously transformed WS2812B frame is in the process of being
    // serialized to the LED ring. Right now the CPU does nothing while a
    // WS2812B frame is being serialized.
    if !**r.BUSY {
        r.RGB_ARRAY.copy_from_slice(&*r.RX_BUFFER.borrow());

        **r.BUSY = true;

        rtfm::set_pending(Interrupt::EXTI0);
    }

    serial.read_exact(r.DMA1, r.RX_BUFFER).unwrap();
}

task!(EXTI0, frame_start);

fn frame_start(_t: Threshold, r: EXTI0::Resources) {
    **r.CONTEXT_SWITCHES += 1;

    let pwm = Pwm(r.TIM2);

    // Construct and send WS2812B frame
    for (rgb, bits) in r.RGB_ARRAY
        .chunks(3)
        .zip(r.WS2812B_BUFFER.borrow_mut().chunks_mut(24))
    {
        let r = rgb[0];
        let g = rgb[1];
        let b = rgb[2];

        // NOTE these LEDs use the GRB format
        for (mut byte, bits) in [g, r, b]
            .iter()
            .cloned()
            .zip(bits.chunks_mut(8))
        {
            for bit in bits.iter_mut().rev() {
                *bit = if byte & 1 == 0 { _0 } else { _1 };

                byte = byte >> 1;
            }
        }
    }

    pwm.set_duties(r.DMA1, Channel::_1, r.WS2812B_BUFFER)
        .unwrap();
}

task!(DMA1_CHANNEL2, frame_tail_start);

fn frame_tail_start(_t: Threshold, r: DMA1_CHANNEL2::Resources) {
    **r.CONTEXT_SWITCHES += 1;

    let timer = Timer(r.TIM1);

    r.WS2812B_BUFFER.release(r.DMA1).unwrap();

    timer.resume();
    timer.restart();
}

task!(TIM1_UP_TIM10, frame_end);

fn frame_end(_t: Threshold, r: TIM1_UP_TIM10::Resources) {
    **r.CONTEXT_SWITCHES += 1;

    let timer = Timer(r.TIM1);

    timer.wait().unwrap();

    timer.pause();

    **r.BUSY = false;
    **r.FRAMES += 1;
}
