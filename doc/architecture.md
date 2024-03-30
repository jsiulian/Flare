# Flare Architecture Description

This document describes the general code structure of Flare.
After reading it, you should have the knowledge to quickly find parts of the code you are interested in, helping you to contribute code changes.

Note that this document may not always be up-to-date with the details, but the high-level description should not change too much in the future of Flare.

The code itself is mostly split up into two parts, the backend code in `src/backend` which mostly wraps [presage](https://github.com/whisperfish/presage) types and includes the core functionality, and the GUI code in `src/gui` which uses the backend objects to display then in the GTK UI.

## Backend

As previously mentioned, the backend code in `src/backend` defines objects wrapping [presage][presage] types, like `Contact`, `Channel` or `Attachment`.
They also provide core functionality like sending the messages or loading more messages.

Other important objects in this regard are also in `src/backend/message`, which defines different message-objects for different message types.
This includes normal text-messages, but also call messages, reactions or deletion-messages.
It is important to note that not every message needs to be displayed in the GUI, e.g. reaction messages do not appear in the timeline of the chat itself but are applied to `TextMessage`s instead.
Messages that are display in the GUI inherit from `DisplayMessage`, which defines how they will be described in notifications or in the sidebar.

The chat history itself is stored as a `Timeline` in `src/backend/timeline`, which is an ordered implementation of `ListStore` with additional functionality like showing or hiding message headers.
The `Timeline` holds objects of types `TimelineItem`, which is anything with a timestamp and with some additional functionality.

Probably the most important type of Flare is the `Manager`.
This contains the more low-level logic, e.g. sending a certain message to a certain contact.
The manager is also responsible for setting up the encrypted storage of Flare and steps to take on how to set up the device (e.g. show a QR code to link the device).
Pretty much every backend type, and also many widgets, hold a reference to the manager to call relevant functionality.
The manager itself is a wrapper around `ManagerThread`, which defines its own thread holding the [`presage::Manager`](https://whisperfish.github.io/presage/presage/manager/struct.Manager.html) which receives message and executes function calls on its internal `presage::Manager`.
The `ManagerThread` also has some further functionality like restarting the websockets once the device went online again after being offline, caching some variables and doing some light error handling.

## GUI

![Annotated GUI](https://gitlab.com/schmiddi-on-mobile/flare/-/raw/master/doc/gui-annotated.png)

All widgets Flare uses are in `src/gui`.
An overview of the most important widgets can be seen in the above picture.

Widgets should have only little logic, mostly related to the widget itself like setting some properties of the UI.
The UI is defined with the [blueprint](https://jwestman.pages.gitlab.gnome.org/blueprint-compiler/) markup language, which definitions can be found in `data/resources/ui`.
The widgets mostly hold a reference to the corresponding backend type, which is defined as a property of the widget itself.
This allows one to define expressions in the blueprint-files which use properties of the backend types, which leads to less code in the widgets.
Also note the `Utility`-object, which defines commonly used functions in expressions, like boolean `and`, `or` and `not`.

Some additional smaller widgets and general objects are defined in `src/gui/components/`, those were in some case ported from other applications to work with Flare.

An important note is that when introducing a new UI widget, multiple places need to be changed:

- `data/resources/ui/_.blp`: Define the blueprint file.
- `data/resources/meson.build`: Add the blueprint file to the list of files to convert to UI files.
- `data/resources/resources.gresources.xml`: Add the resulting UI file to the resources.
- `src/gui/_.rs`: Define the code corresponding to the widget.
- `src/gui/mod.rs`: Define your new file as a module to be usable in other code.
- (sometimes required): Call `ensure_type` on your class in the parent object `class_init`.

Another important note is that while Flare uses a lot of expressions in the blueprint-files, those can lead to very unexpected behavior in rare cases, like crashes.
In those cases, set the properties in the Rust-code instead.
