#![deny(warnings)]
#![feature(const_fn)]
#![feature(used)]
#![no_std]

extern crate blue_pill;

extern crate cortex_m_rt;

#[macro_use]
extern crate cortex_m_rtfm as rtfm;

extern crate embedded_hal as hal;

extern crate shared;

use core::cell::{Cell, RefCell};

use blue_pill::dma::{Buffer, Dma1Channel2, Dma1Channel4, Dma1Channel5};
use blue_pill::stm32f103xx;
use blue_pill::time::{Hertz, Microseconds};
use blue_pill::{Channel, Pwm, Serial, Timer};
use hal::prelude::*;
use rtfm::{C1, Resource, P0, P1, T0, T1, TMax};
use shared::State;
use stm32f103xx::interrupt::{DMA1_CHANNEL2, DMA1_CHANNEL4, DMA1_CHANNEL5,
                             EXTI0, TIM1_UP_TIM10, TIM3};

// CONFIGURATION
const _0: u8 = 3;
const _1: u8 = 7;
const BAUD_RATE: Hertz = Hertz(115_200);
const LATCH_DELAY: Microseconds = Microseconds(50);
const LOG_FREQUENCY: Hertz = Hertz(1);
const WS2812B_FREQUENCY: Hertz = Hertz(400_000);

// RESOURCES
peripherals!(stm32f103xx, {
    AFIO: Peripheral { ceiling: C0, },
    DMA1: Peripheral { ceiling: C1, },
    DWT: Peripheral { ceiling: C1, },
    GPIOA: Peripheral { ceiling: C0, },
    RCC: Peripheral { ceiling: C0, },
    TIM1: Peripheral { ceiling: C1, },
    TIM2: Peripheral { ceiling: C1, },
    TIM3: Peripheral { ceiling: C1, },
    USART1: Peripheral { ceiling: C1, },
});

static BUSY: Resource<Cell<bool>, C1> = Resource::new(Cell::new(false));
static CONTEXT_SWITCHES: Resource<Cell<u16>, C1> = Resource::new(Cell::new(0));
static FRAMES: Resource<Cell<u8>, C1> = Resource::new(Cell::new(0));
static RGB_ARRAY: Resource<RefCell<[u8; 24 * 3]>, C1> =
    Resource::new(RefCell::new([0; 24 * 3]));
static TX_BUFFER: Resource<Buffer<[u8; 13], Dma1Channel4>, C1> =
    Resource::new(Buffer::new([0; 13]));
static RX_BUFFER: Resource<Buffer<[u8; 24 * 3], Dma1Channel5>, C1> =
    Resource::new(Buffer::new([0; 24 * 3]));
static SLEEP_CYCLES: Resource<Cell<u32>, C1> = Resource::new(Cell::new(0));
static WS2812B_BUFFER: Resource<Buffer<[u8; (24*24) + 1], Dma1Channel2>, C1> =
    Resource::new(Buffer::new([0; (24 * 24) + 1]));

// INITIALIZATION
fn init(ref prio: P0, thr: &TMax) {
    let afio = &AFIO.access(prio, thr);
    let dma1 = &DMA1.access(prio, thr);
    let dwt = DWT.access(prio, thr);
    let gpioa = &GPIOA.access(prio, thr);
    let rcc = &RCC.access(prio, thr);
    let rx_buffer = RX_BUFFER.access(prio, thr);
    let tim1 = TIM1.access(prio, thr);
    let tim2 = TIM2.access(prio, thr);
    let tim3 = TIM3.access(prio, thr);
    let usart1 = USART1.access(prio, thr);

    let timer1 = Timer(&*tim1);
    let timer3 = Timer(&*tim3);
    let serial = Serial(&*usart1);
    let pwm = Pwm(&*tim2);

    dwt.enable_cycle_counter();

    timer1.init(LATCH_DELAY, rcc);

    timer3.init(LOG_FREQUENCY.invert(), rcc);

    serial.init(BAUD_RATE.invert(), afio, Some(dma1), gpioa, rcc);

    pwm.init(WS2812B_FREQUENCY.invert(), afio, Some(dma1), gpioa, rcc);
    pwm.enable(Channel::_1);

    serial.read_exact(dma1, rx_buffer).unwrap();

    timer3.resume();
}

// IDLE LOOP
fn idle(ref prio: P0, _: T0) -> ! {
    loop {
        rtfm::atomic(|thr| {
            let dwt = DWT.access(prio, thr);
            let sleep_cycles = SLEEP_CYCLES.access(prio, thr);

            // Sleep
            let before = dwt.cyccnt.read();
            rtfm::wfi();
            let after = dwt.cyccnt.read();

            let elapsed = after.wrapping_sub(before);
            sleep_cycles.set(sleep_cycles.get() + elapsed);
        });

        // Service interrupts
    }
}

// TASKS
tasks!(stm32f103xx, {
    frame_start: Task {
        interrupt: EXTI0,
        priority: P1,
        enabled: true,
    },
    frame_tail_start: Task {
        interrupt: DMA1_CHANNEL2,
        priority: P1,
        enabled: true,
    },
    frame_end: Task {
        interrupt: TIM1_UP_TIM10,
        priority: P1,
        enabled: true,
    },
    log: Task {
        interrupt: TIM3,
        priority: P1,
        enabled: true,
    },
    rx: Task {
        interrupt: DMA1_CHANNEL5,
        priority: P1,
        enabled: true,
    },
    tx_transfer_done: Task {
        interrupt: DMA1_CHANNEL4,
        priority: P1,
        enabled: true,
    },
});

fn log(_task: TIM3, ref prio: P1, ref thr: T1) {
    let context_switches = CONTEXT_SWITCHES.access(prio, thr);
    context_switches.set(context_switches.get() + 1);

    let dma1 = &DMA1.access(prio, thr);
    let dwt = DWT.access(prio, thr);
    let frames = FRAMES.access(prio, thr);
    let sleep_cycles = SLEEP_CYCLES.access(prio, thr);
    let tim3 = TIM3.access(prio, thr);
    let tx_buffer = TX_BUFFER.access(prio, thr);
    let usart1 = USART1.access(prio, thr);

    let timer = Timer(&*tim3);
    let serial = Serial(&*usart1);

    timer.wait().unwrap();

    let snapshot = dwt.cyccnt.read();
    let state = State {
        context_switches: context_switches.get(),
        frames: frames.get(),
        sleep_cycles: sleep_cycles.get(),
        snapshot: snapshot,
    };
    state.serialize(&mut *tx_buffer.borrow_mut());

    serial.write_all(dma1, tx_buffer).unwrap();

    context_switches.set(0);
    frames.set(0);
    sleep_cycles.set(0);
}

fn tx_transfer_done(_task: DMA1_CHANNEL4, ref prio: P1, ref thr: T1) {
    let context_switches = CONTEXT_SWITCHES.access(prio, thr);
    context_switches.set(context_switches.get() + 1);

    let dma1 = &DMA1.access(prio, thr);
    let tx_buffer = TX_BUFFER.access(prio, thr);

    tx_buffer.release(dma1).unwrap();
}

fn rx(_task: DMA1_CHANNEL5, ref prio: P1, ref thr: T1) {
    let context_switches = CONTEXT_SWITCHES.access(prio, thr);
    context_switches.set(context_switches.get() + 1);

    let busy = BUSY.access(prio, thr);
    let dma1 = &DMA1.access(prio, thr);
    let rgb_array = RGB_ARRAY.access(prio, thr);
    let rx_buffer = RX_BUFFER.access(prio, thr);
    let usart1 = USART1.access(prio, thr);

    let serial = Serial(&*usart1);

    rx_buffer.release(dma1).unwrap();

    // When busy we just ignore incoming frames
    // TODO we can probably double throughput if we turn this into a pipeline
    // where an incoming RGB frame is transformed into a WS2812B frame while a
    // previously transformed WS2812B frame is in the process of being
    // serialized to the LED ring. Right now the CPU does nothing while a
    // WS2812B frame is being serialized.
    if !busy.get() {
        rgb_array.borrow_mut().copy_from_slice(&*rx_buffer.borrow());

        busy.set(true);

        rtfm::request(frame_start);
    }

    serial.read_exact(dma1, rx_buffer).unwrap();
}

fn frame_start(_task: EXTI0, ref prio: P1, ref thr: T1) {
    let context_switches = CONTEXT_SWITCHES.access(prio, thr);
    context_switches.set(context_switches.get() + 1);

    let dma1 = &DMA1.access(prio, thr);
    let rgb_array = RGB_ARRAY.access(prio, thr);
    let tim2 = TIM2.access(prio, thr);
    let ws2812b_buffer = WS2812B_BUFFER.access(prio, thr);

    let pwm = Pwm(&*tim2);

    // Construct and send WS2812B frame
    for (rgb, bits) in rgb_array
        .borrow()
        .chunks(3)
        .zip(ws2812b_buffer.borrow_mut().chunks_mut(24))
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

    pwm.set_duties(dma1, Channel::_1, ws2812b_buffer).unwrap();
}

fn frame_tail_start(_task: DMA1_CHANNEL2, ref prio: P1, ref thr: T1) {
    #![allow(unreachable_code)]

    let context_switches = CONTEXT_SWITCHES.access(prio, thr);
    context_switches.set(context_switches.get() + 1);

    let dma1 = &DMA1.access(prio, thr);
    let tim1 = TIM1.access(prio, thr);
    let ws2812b_buffer = WS2812B_BUFFER.access(prio, thr);

    let timer = Timer(&*tim1);

    ws2812b_buffer.release(dma1).unwrap();

    timer.restart();
    timer.resume();
}

fn frame_end(_task: TIM1_UP_TIM10, ref prio: P1, ref thr: T1) {
    let busy = BUSY.access(prio, thr);
    let frames = FRAMES.access(prio, thr);
    let tim1 = TIM1.access(prio, thr);

    let timer = Timer(&*tim1);

    timer.wait().unwrap();

    timer.pause();

    busy.set(false);
    frames.set(frames.get() + 1);
}
