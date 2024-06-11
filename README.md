# tawaylon

Experimenting with Talon-like features in Wayland.

Challenges:

- **Streaming voice recognition.** And not just that, but for not-normal-speech: a model trained on sentences may not get "air bat cap deck" right.
- **State collection.** Wayland, AT-SPI2, and other protocols for seeing where window are, what is / can be selected, etc.
- **Decision logic.** Parsing commands, reconciling with current state, and determining next action.
- **Output.** Wayland, AT-SPI2, and other protocols -- up to and including a virtual tablet & keyboard -- to get data back down to applications.

