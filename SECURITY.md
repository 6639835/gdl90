
# Security policy

## Reporting

Report parser crashes, unbounded memory growth, packet-boundary confusion, integer overflow, or protocol validation bypasses through a private security advisory in this repository. Include the smallest reproducing byte sequence and the affected API.

## Supported input assumptions

All public byte-decoding APIs must tolerate malformed input without panic. Live network helpers additionally enforce bounded frame and datagram sizes. Callers should keep the defaults unless a larger limit is justified and separately tested.

## Aviation disclaimer

Security fixes and passing tests do not make this project certified avionics. Do not use it as the sole source of traffic, terrain, weather, navigation, or collision-avoidance information.
