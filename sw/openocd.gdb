target extended-remote /dev/ttyACM0
monitor swdp_scan
attach 1
file ./target/thumbv6m-none-eabi/release/nixiewatch
load
break main.rs 76
start
set mem inaccessible-by-default off
