# `firmware`

> LED driver firmware

## Wiring

- Connect pin PA0 to the LED ring data in pin.
- PA9 is the TX pin of the serial interface and PA10 is the RX pin.
- All components, the blue pill, the LED ring and the HC-06, require a power
  supply voltage of 5V.

## Communication protocol

The serial interface of the LED driver operates at 115200 bps. The driver
expects 72-byte frames over this interface. This frame represents the RGB color,
1 byte per channel, of each of the 24 LEDs. The driver will update the color
value of the 24 LEDs every time a new 72-byte frame arrives.

## Operation

### [WS2812B]

[WS2812B]: http://www.seeedstudio.com/document/pdf/WS2812B%20Datasheet.pdf

Every WS2812B LED actually contains 3 LEDs: a red one, a green one and a blue
one, plus an integrated control circuit that controls the brightness of each of
these LEDs. The final RGB color of a single WS2812B LED can be controlled via an
asynchronous serial interface that expects a 24-bit frame. The frame is
essentially an RGB pixel, one byte per channel, encoded in the following GRB
format:

```
G7 G6 G5 G4 G3 G2 G1 G0 R7 R6 R5 R4 R3 R2 R1 R0 B7 B6 B5 B4 B3 B2 B1 B0
```

Where `Xn` is a single bit; `G7` is the most significant bit of the green
brightness value; `R0` is the least significant bit of the red brightness value;
etc.

The serial interface uses a Non Return to Zero (NRZ) encoding scheme where both
the `0` value and the `1` value are represented by a (periodic) square wave
signal. The difference between a `0` and a `1` is the duration of the high state
of the signal.

A `0` value:

``` text
+-----+
|     |
|     +--------------
```

A `1` value:

``` text
+------------+
|            |
|            +-------
```

A `0b10` value (the most significant bit is sent first):

``` text
+-----+             +------------+
|     |             |            |
|     +-------------+            +-------
```

The main feature of the WS2812B LEDs is that they can be daisy chained and still
be controlled using a single control signal. For example when two WS2812B LEDs
are chained as shown below:

``` text
--------+---------------+---------------- 5V
        |               |
        |      DOUT     |      DOUT
        +-------+---+   +-------+
        |       |   |   |       |
        |WS2812B|   |   |WS2812B|
        |       |   |   |       |
      ->+-------+   +-->+-------+
       DIN      |      DIN      |
                |               |
----------------+---------------+-------- GND
```

and a 48-bit frame is sent the first WS2812B LED (the left one) will keep the
first 24 bits of the frame and pass the next 24 bits of the frame to the next
WS2812B LED. In this scenario if you sent `0x0000FF` (most significant bit
first) followed by `0xFF0000` as the 48-bit frame the first LED would turn BLUE
and the second LED would turn GREEN (remember: GRB format).

To mark the end of the frame and force the WS2812B LEDs to update their color
the data line must be held low for 50 microseconds.

### PWM Interface

To control the LED ring the LED driver uses its PWM (Pulse Width Modulation)
functionality to generate the control signal. As per the datasheet a `0` value
should have a high time of around 350 ns (nanoseconds), and a `1` value should
have a high time of around 900 ns. The low timer of the PWM signal is not
important as long as the period of the PWM is at least 1250 ns.

To meet these requirements a PWM signal with a frequency of 400 KHz (period =
2500 ns) was selected (\*). At a timer frequency of 8 MHz this resulted in a
prescaler value of 1 where each timer tick equals 125 ns (1 / 8e6). A duty cycle
(compare output) value of 3 generates a high time of 375 which is close to the
high time required for a WS2812B `0` value, and a duty cycle value of 7
generates a high time of 875 ns which is close to the high time required for a
WS2812B `1` value.

(\*) WS2812B LEDs can operate at a frequency of 800 KHz but this value wasn't
achievable with the selected hardware.

To use the PWM functionality to generate the control signal the duty cycle value
would need to change every 2500 ns which is the PWM signal period. This timing
constraint is too tight to have the CPU do the duty cycle update so this task
is delegated to the DMA (Direct Memory Access) peripheral.

### DMA operation

The LED driver DMA is configured to change the duty cycle of the PWM peripheral
every time a period of the PWM signal ends. The values that the DMA uses come
from a 577-byte buffer. Each byte of the buffer maps to one WS2812B bit value.
Except for the last byte of the buffer which is always kept at value 0 to
produce the low signal required to mark the end of the WS2812B frame.

### Marshaling

The LED driver expects a 72-byte frame that represents the RGB values of the 24
LEDs on the serial interface. This 72-byte frame needs to be converted into the
577-byte buffer that's used with the DMA. This conversion is the main CPU user;
the DMA transfer doesn't make use of CPU time.

## Performance

At a core clock frequency of 8 MHz and using a PWM signal of 200 KHz the LED
driver can redraw the whole LED ring at a maximum of 160 frames per second using
15% of CPU time.

``` console
$ # send 1000 random frames to the LED driver
$ dd if=/dev/urandom of=/dev/rfcomm0 bs=72000 count=1
```

``` console
$ cat /dev/rfcomm0 | logcat
CPU: 0.00% - CS: 2, F: 0
CPU: 14.66% - CS: 466, F: 154
CPU: 15.06% - CS: 479, F: 159
CPU: 15.05% - CS: 480, F: 160
CPU: 15.13% - CS: 481, F: 159
CPU: 15.06% - CS: 479, F: 159
CPU: 15.05% - CS: 480, F: 160
CPU: 4.64% - CS: 149, F: 49
CPU: 0.00% - CS: 2, F: 0
```
