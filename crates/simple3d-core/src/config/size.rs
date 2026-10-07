//! Which part of an object stays put while it is resized or scaled (issue 120).

use serde::{Deserialize, Deserializer, Serialize};

/// What holds still while an object grows or shrinks, on every axis alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeAnchor {
    /// The side across from the pulled handle; a typed size or a key grows about the centre, since
    /// neither pulls a side.
    #[default]
    Opposite,
    Centre,
}

impl SizeAnchor {
    pub const ALL: [SizeAnchor; 2] = [SizeAnchor::Opposite, SizeAnchor::Centre];

    pub fn label(self) -> &'static str {
        match self {
            SizeAnchor::Opposite => "Opposite side",
            SizeAnchor::Centre => "Centre",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SizeAnchor::Opposite => "Only the pulled side moves; the side across from it stays in place.",
            SizeAnchor::Centre => "The centre stays in place; both sides move by the same amount.",
        }
    }

    /// Reads anything it does not know, such as the per-axis anchors of an earlier build, as the
    /// default rather than failing the whole settings file.
    pub fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<SizeAnchor, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Known(SizeAnchor),
            Other(serde::de::IgnoredAny),
        }
        Ok(match Stored::deserialize(deserializer)? {
            Stored::Known(anchor) => anchor,
            Stored::Other(_) => SizeAnchor::default(),
        })
    }
}
