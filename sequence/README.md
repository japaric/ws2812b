# `sequence`

> Tool to send commands to the LED driver

## Usage

``` console
$ sequence single FF0000 > /dev/rfcomm0
```

- `/dev/rfcomm0` is an RFCOMM port paired with the LED driver.
- `single FF0000` is a LED driver command. All the commands are listed below.

### `single`

The `single` command will set all the LEDs to the same color. This command takes
one argument which is the color all the LEDs will be set to.

``` console
$ sequence single 0f000f > /dev/rfcomm0
```

![Single sequence](/assets/single.jpg)

### `random`

The `random` command will set each LED to a random color. The frame will be
updated (all the colors will be randomized again) at the rate indicated by the
`-f`, the FPS, flag.

``` console
$ sequence random -f1 > /dev/rfcomm0
```

![Random sequence](/assets/random.jpg)

### `roulette`

The `roulette` command simulates a (never ending) roulette game where a single
LED represents the roulette ball. The `-f`, the FPS, flag specifies the spin
speed of the roulette ball and the first argument specifies the color of the
roulette ball.

``` console
$ sequence roulette -f16 521900 > /dev/rfcomm0
```

![Roulette sequence](/assets/roulette.gif)

### `crescendo`

The `crescendo` command recreates the animation shown below:

![Crescendo sequence](/assets/crescendo.gif)

``` console
$ sequence crescendo -f64 0f000f
```

The `-f`, FPS, flag specifies the speed of the animation and the first argument
specifies the color of the LEDs.
