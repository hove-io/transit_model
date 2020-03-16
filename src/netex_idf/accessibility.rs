// Copyright (C) 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

use crate::{minidom_utils::TryOnlyChild, objects::Availability};
use minidom::Element;

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct Accessibility {
    pub wheelchair: Availability,
    pub visual_announcement: Availability,
    pub audible_announcement: Availability,
}

pub fn accessibility(el: &Element) -> Option<Accessibility> {
    fn availability(val: &str) -> Availability {
        match val {
            "true" => Availability::Available,
            "false" => Availability::NotAvailable,
            _ => Availability::InformationNotAvailable,
        }
    }

    let mobility_impaired_access = el.only_child("MobilityImpairedAccess")?.text();
    let limitation = el
        .only_child("limitations")?
        .only_child("AccessibilityLimitation")?;
    let visual_signs_available = limitation.only_child("VisualSignsAvailable")?.text();
    let audio_signs_available = limitation.only_child("AudibleSignalsAvailable")?.text();

    Some(Accessibility {
        wheelchair: availability(&mobility_impaired_access),
        visual_announcement: availability(&visual_signs_available),
        audible_announcement: availability(&audio_signs_available),
    })
}
