# `logcat`

> Decodes the robot log frames

## Usage

``` console
$ cat /dev/rfcomm0 | logcat
CPU: 0.00% - CS: 2, F: 0
CPU: 0.00% - CS: 2, F: 0
```

- `/dev/rfcomm0` is an RFCOMM port paired with the LED driver.
- `CPU: 0.00%` is the CPU usage of the LED driver
- `CS: 2` is the number of context switches the LED driver did since the last
  log data point.
- `F: 0` is the number of frames the LED driver drew since the last log data
  point.
