
This package exists to generate Rust code for nonstandard Wayland protocols
provided by wlroots and sway:

The [wayland-rs] project uses [wlr-protocols] as its upstream.
But that does not include all of the [protocols offered in wlroots][wlroots-protocol];
in particular, the virtual keyboard protocol is not present.

Luckily, we can use [wayland-scanner] to generate our own -- here.

[wlroots-protocol]: https://gitlab.freedesktop.org/wlroots/wlroots/-/tree/master/protocol
[wlr-protocols]: https://gitlab.freedesktop.org/wlroots/wlr-protocols
[wayland-rs]: https://github.com/Smithay/wayland-rs/
[wayland-scanner]: https://crates.io/crates/wayland-scanner
