/// Control a virtual keyboard:
/// have the Wayland client report keyboard events to the compositor.
pub mod virtual_keyboard {

    #[allow(missing_docs)]
    pub mod v1 {
        pub mod client {
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;

                wayland_scanner::generate_interfaces!(
                    "./protocol/virtual-keyboard-unstable-v1.xml"
                );
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!("./protocol/virtual-keyboard-unstable-v1.xml");
        }
    }
}
