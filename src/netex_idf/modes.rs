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

use lazy_static::lazy_static;
use std::collections::HashMap;

pub struct ModeNetexIDF {
    // Tuple (mode_id, mode_name)
    pub physical_mode: (&'static str, &'static str),
    pub commercial_mode: (&'static str, &'static str),
}

lazy_static! {
    pub static ref MODES: HashMap<&'static str, ModeNetexIDF> = {
        let mut m = HashMap::new();
        m.insert(
            "air",
            ModeNetexIDF {
                physical_mode: ("Air", "Avion"),
                commercial_mode: ("Air", "Avion"),
            },
        );
        m.insert(
            "bus",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m.insert(
            "coach",
            ModeNetexIDF {
                physical_mode: ("Coach", "Autocar"),
                commercial_mode: ("Coach", "Autocar"),
            },
        );
        m.insert(
            "ferry",
            ModeNetexIDF {
                physical_mode: ("Ferry", "Ferry"),
                commercial_mode: ("Ferry", "Ferry"),
            },
        );
        m.insert(
            "metro",
            ModeNetexIDF {
                physical_mode: ("Metro", "Métro"),
                commercial_mode: ("Metro", "Métro"),
            },
        );
        m.insert(
            "rail",
            ModeNetexIDF {
                physical_mode: ("LocalTrain", "Train régional / TER"),
                commercial_mode: ("LocalTrain", "Train régional / TER"),
            },
        );
        m.insert(
            "trolleyBus",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("TrolleyBus", "TrolleyBus"),
            },
        );
        m.insert(
            "tram",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("Tramway", "Tramway"),
            },
        );
        m.insert(
            "water",
            ModeNetexIDF {
                physical_mode: ("Boat", "Navette maritime / fluviale"),
                commercial_mode: ("Boat", "Navette maritime / fluviale"),
            },
        );
        m.insert(
            "cableway",
            ModeNetexIDF {
                physical_mode: ("Tramway", "Tramway"),
                commercial_mode: ("CableWay", "CableWay"),
            },
        );
        m.insert(
            "funicular",
            ModeNetexIDF {
                physical_mode: ("Funicular", "Funiculaire"),
                commercial_mode: ("Funicular", "Funiculaire"),
            },
        );
        m.insert(
            "lift",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m.insert(
            "other",
            ModeNetexIDF {
                physical_mode: ("Bus", "Bus"),
                commercial_mode: ("Bus", "Bus"),
            },
        );
        m
    };
}
