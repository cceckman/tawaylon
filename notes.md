# Impossible Day

Idea: Talon-like voice control and typing... but that works under Sway (wlroots).

Talon is only (nominally) compatible with the X11 graphical platform. I'm aware of a few reasons for this:

- X11 is looser about input control, focus, etc; any program can access the entire screen at any time. Wayland keeps stricter track of focus.
- X11 has standard protocols for manipulating window state: listing windows, selecting, moving, etc. Wayland compositors don't have a common protocol for this.
- Wayland is missing many of the A11y protocols that can be used for e.g. screen-reader/pointer control; or different compositors have different protocols, or different levels of support.
	- [This repo](https://github.com/splondike/wayland-accessibility-notes) has notes on Wayland Accessibility.
	- AT-SPI2 - accessibility tools access over DBus.


Problems and possible solutions:
- Different compositors, different protocols. We're only looking at Sway, so we're limited by (and also to) what it supports.
- Voice to commands. There's some options here:
	- whisper.cpp has a couple of examples of doing audio capture and live transcription. It looks like `whisper.cpp` has a "callback on text recognized" which could be useful for streaming; though it also looks like it only takes a chunk at a time, i.e. we'd need to find the word boundaries up-front. Also seems like they have examples for that. Uses libsdl.
	- [WhisperLive](https://github.com/collabora/WhisperLive) from Collabora. [faster-whisper](https://github.com/SYSTRAN/faster-whisper) is its backend; which claims to be faster than whisper.cpp. Lots of examples using it.
- State interfaces: Compositor state, application state.
	- This is where we run into the AT-SPI2 issues, and the Wayland protocol support.
	- Sway actually has _pretty good_ non-Wayland protocols for scripting window management; looks like it actually has pretty good support within Wayland too, for stuff like titlebars.
	- [Wayland Protocol browser](https://wayland.app/protocols/) is a good reference here.
	- Not sure how to correlate the compositor state with [AT-SPI2](https://gitlab.gnome.org/GNOME/at-spi2-core), but probably possible?
- Command parsing and customization. This is where we have the most freedom; it's "just code".
- Output.
	- Sway has decent control protocols for window management. Is '[activation](https://wayland.app/protocols/xdg-activation-v1)' what this means?
		- [top-level management](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1), for things like taskbars
	- [This protocol](https://wayland.app/protocols/input-method-unstable-v1) provides text input (client to compositor), but it isn't widely supported. (Not to be confused with the [Text input](https://wayland.app/protocols/text-input-unstable-v3) protocol, which is used for the compositor providing text to the client.) Do we need to create a virtual input device at the kernel layer?
		- wlroots supports [zwp_input_method_v2](https://wayland.app/protocols/input-method-unstable-v2), which "lets the client to serve as an input method for a seat."
	- Pointer input? There's protocols for _events_ for [tablet](https://wayland.app/protocols/tablet-v2) and [relative pointer](https://wayland.app/protocols/relative-pointer-unstable-v1). Do we need to create a virtual input device at the kernel layer?
		- [`wl_seat`](https://wayland.app/protocols/wayland#wl_seat) gives access to `wl_pointer`, `wl_keyboard`, and `wl_touch`. `wl_pointer.set_cursor` (cursor move) only works to move to "your own" surfaces.
		- [WLRoots offers a virtual pointer](https://wayland.app/protocols/wlr-virtual-pointer-unstable-v1) that emulates a physical pointer, including absolute motion. That's how `warpd` works.
	- Prior art for virtual input devices:  [ydotool](https://github.com/ReimuNotMoe/ydotool) apparently opens [`/dev/uinput`](https://kernel.org/doc/html/v4.12/input/uinput.html) as root. [warpd](https://github.com/rvaiya/warpd) works for just mouse motion -- but has to "capture keyboard" via the compositor, i.e. compositor has to launch it.

There's a universe in which the input is handled by a "compositor wrapper", that proxies all the Wayland requests...but every surface is a subsurface of the A11y layer. That's kinda icky though.

## Wayland protocols

https://wayland.app/protocols/ is a great documentation resource. The red/pink methods are client-to-compositor; the green items are events from server to client.

### Differences

- Act as input device
- Draw-over screen -- is possible, but how does it work?
- Window state detection
- Window state manipulation
- Same: AT-SPI2

### Usage

https://github.com/NilsBrause/waylandpp ? Does it have protocols for all of them? Maybe not, maybe we can add tothem. I'm assuming this is code-gen? Maybe not.

*Someone* should have one? Ah, `wayland-scanner` looks like The Thing: generates C ABI, [here](https://wayland-book.com/libwayland/wayland-scanner.html). [This](https://github.com/Smithay/wayland-rs) implements a `wayland-scanner` for Rust; already have bindings in other crates, e.g. [WLR](https://docs.rs/wayland-protocols-wlr/latest/wayland_protocols_wlr/)


## Talon on Wayland

It _kinda_ works. It interacts oddly with XWayland.
- Can't `window close`
- Sometimes another XWayland window gets "altered" when I do something in Talon; mostly when I get a notification, maybe? It uses `notibfy-send` behavior.
- I'm getting a fair amount of noise; keeps hearing "bat" when I'm typing.

but I'm gonna type like this for a little bit. It didn't work great; and the "correction" modes are tricky / annoying. Bleh. Practice? Better mic?

### Talon: inherent vs. plugin

What things are in the Talon core (closed-source) vs. in plugins?

One way to approach: what things are [part of the API](https://talonvoice.com/docs/index.html#overview)
vs. part of the [the plugin set](https://github.com/talonhub/community)?

- Clipboard: [API](https://talonvoice.com/docs/index.html#talon.Context.apps)
- Window state: API -- [app id](https://talonvoice.com/docs/index.html#talon.Context.apps)
  - VSCode under Wayland: Talon can't see it in plugins to match context
  - VSCode under XWayland: Talon can see it, activate context
- Window manipulation: [community](https://github.com/talonhub/community/tree/main/core/windows_and_tabs)

## whisper.cpp stream example

Took a while to pick things up. VAD only detected the first sample.

## faster-whisper attempts

### whisper_streaming

[this](https://github.com/ufal/whisper_streaming) says:

> Default Whisper is intended for audio chunks of at most 30 seconds that contain one full sentence. Longer audio files must be split to shorter chunks and merged with "init prompt". In low latency simultaneous streaming mode, the simple and naive chunking fixed-sized windows does not work well, it can split a word in the middle. It is also necessary to know when the transcribt is stable, should be confirmed ("commited") and followed up, and when the future content makes the transcript clearer.

Which makes sense, that does seem to be how it's doing it. Use "future info" to find the new data.

So I guess we'd need to be pretty specific about how we're doing VAD -- utterance detection, end-of-utterance matching, and assumption of independence. A relatively short utterance length -- or "over?" or "click"? -- to pick it up.
### WhisperLive

`pip install whisper-live` winds up...installing 780MiB of PyTorch. Great! Love it. Isn't `pip` great? And virtual environments! And it includes another >1GiB of nVidia stuff, even though _I don't have any nVidia hardware on my machine!_

Takes 5.8GiB to install. That's not workable. So yeah, this is something to compare against but not something to actually use.

Client/server model. Works pretty well for text, saying "air bat cap sit" got me "airbag capsit" which is not quite what we're looking for.

## wav2letter

This is what Talon uses; now part of Facebook's Flashlight app: [here](https://github.com/flashlight/flashlight/blob/main/flashlight/app/asr/Decode.cpp)

That's a lot less "user friendly" than the Whisper stuff; but may be better for this!

## Sepia STT

https://github.com/SEPIA-Framework/sepia-stt-server/blob/master/python-client/example.py

This is pretty ok; there's quasi-final results, looks like, if the "best transcription" is empty (presumably for the next result?)
So it does wind up splitting utterances

By default wraps Vosk

## Vosk

...which sits on top of a fork of Kaldi, apparently?

Says Vosk is "offline" but it may be ported to online ~easily, algorithm:

- Append to your audio buffer
- Re-run inference
- If in the last N seconds, adding new audio didn't change the inference, "commit" and flush

https://github.com/Bear-03/vosk-rs

"new with grammar", can be constrained

