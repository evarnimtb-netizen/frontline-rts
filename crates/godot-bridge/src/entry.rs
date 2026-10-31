#![allow(unsafe_code)]
use godot::prelude::*;
struct FrontlineExtension;
#[gdextension]
unsafe impl ExtensionLibrary for FrontlineExtension {}
