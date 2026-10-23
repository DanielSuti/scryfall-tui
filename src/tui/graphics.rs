//! Which graphics protocol the terminal supports for card images, found as Yazi finds its
//! image adapter: hints in the environment first, then questions to the terminal about the
//! kitty graphics protocol, sixel support and the cell size in pixels.

use ratatui_image::picker::{Picker, ProtocolType};

/// Asks the terminal which graphics protocol it supports and how large its cells are. Returns
/// `None` when it supports none of kitty's, sixel or iTerm2's, or does not answer.
///
/// Call it in raw mode, before anything else reads the terminal's input: the answers arrive
/// there. A terminal that doesn't answer delays the start by up to two seconds.
pub fn detect() -> Option<Picker> {
    Picker::from_query_stdio()
        .ok()
        .filter(|picker| supported(picker.protocol_type()))
}

/// Returns true for the protocols that draw real images. Half blocks, the fallback without a
/// protocol, are too coarse for card text and are not used.
pub fn supported(protocol: ProtocolType) -> bool {
    match protocol {
        ProtocolType::Kitty | ProtocolType::Sixel | ProtocolType::Iterm2 => true,
        ProtocolType::Halfblocks => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_image_protocols_are_supported() {
        assert!(supported(ProtocolType::Kitty));
        assert!(supported(ProtocolType::Sixel));
        assert!(supported(ProtocolType::Iterm2));
        assert!(!supported(ProtocolType::Halfblocks));
    }
}
