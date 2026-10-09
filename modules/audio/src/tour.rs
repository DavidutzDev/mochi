//! This module's steps in the tour: its views with made-up data, and the
//! release each feature came in. See the tour module.

use mochi_core::ContributionSpec;
use serde_json::json;

pub fn steps() -> Vec<ContributionSpec> {
    vec![
        ContributionSpec::new("tour", "step", "mixer", "Panel", "The mixer")
            .icon("volume_up")
            .order(40)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.5",
                "caption": "The mixer: your outputs and inputs, up to 150% if you like, and every app with its own volume.",
                "payload": {
                    "connected": true,
                    "max_volume": 150,
                    "output": {
                        "name": "out",
                        "description": "Arctis Nova 7",
                        "volume": 62,
                        "muted": false,
                        "icon": "headset",
                        "default": true
                    },
                    "input": {
                        "name": "in",
                        "description": "Webcam microphone",
                        "volume": 80,
                        "muted": false,
                        "icon": "microphone",
                        "default": true
                    },
                    "outputs": [
                        {
                            "name": "out",
                            "description": "Arctis Nova 7",
                            "volume": 62,
                            "muted": false,
                            "icon": "headset",
                            "default": true
                        },
                        {
                            "name": "speakers",
                            "description": "Speakers",
                            "volume": 40,
                            "muted": false,
                            "icon": "speaker",
                            "default": false
                        }
                    ],
                    "inputs": [
                        {
                            "name": "in",
                            "description": "Webcam microphone",
                            "volume": 80,
                            "muted": false,
                            "icon": "microphone",
                            "default": true
                        }
                    ],
                    "apps": [
                        {
                            "id": "spotify",
                            "name": "Spotify",
                            "title": "Teardrop",
                            "icon": "spotify",
                            "volume": 70,
                            "muted": false,
                            "playing": true,
                            "output": "out",
                            "streams": [
                                {
                                    "id": "42",
                                    "title": "Teardrop",
                                    "volume": 70,
                                    "muted": false,
                                    "playing": true,
                                    "output": "out"
                                }
                            ]
                        },
                        {
                            "id": "firefox",
                            "name": "Firefox",
                            "title": "A video",
                            "icon": "firefox",
                            "volume": 100,
                            "muted": true,
                            "playing": false,
                            "output": "out",
                            "streams": []
                        }
                    ]
                }
            })),
        ContributionSpec::new("tour", "step", "apps", "Panel", "Apps in the mixer")
            .icon("volume_up")
            .order(41)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.7",
                "caption": "Each app has one row with a meter of its sound and a button to move it to another output; its streams are inside.",
                "payload": {
                    "connected": true,
                    "max_volume": 150,
                    "output": {
                        "name": "out",
                        "description": "Arctis Nova 7",
                        "volume": 62,
                        "muted": false,
                        "icon": "headset",
                        "default": true
                    },
                    "input": {
                        "name": "in",
                        "description": "Webcam microphone",
                        "volume": 80,
                        "muted": false,
                        "icon": "microphone",
                        "default": true
                    },
                    "outputs": [
                        {
                            "name": "out",
                            "description": "Arctis Nova 7",
                            "volume": 62,
                            "muted": false,
                            "icon": "headset",
                            "default": true
                        },
                        {
                            "name": "speakers",
                            "description": "Speakers",
                            "volume": 40,
                            "muted": false,
                            "icon": "speaker",
                            "default": false
                        }
                    ],
                    "inputs": [
                        {
                            "name": "in",
                            "description": "Webcam microphone",
                            "volume": 80,
                            "muted": false,
                            "icon": "microphone",
                            "default": true
                        }
                    ],
                    "apps": [
                        {
                            "id": "spotify",
                            "name": "Spotify",
                            "title": "Teardrop",
                            "icon": "spotify",
                            "volume": 70,
                            "muted": false,
                            "playing": true,
                            "output": "out",
                            "streams": [
                                {
                                    "id": "42",
                                    "title": "Teardrop",
                                    "volume": 70,
                                    "muted": false,
                                    "playing": true,
                                    "output": "out"
                                }
                            ]
                        },
                        {
                            "id": "firefox",
                            "name": "Firefox",
                            "title": "A video",
                            "icon": "firefox",
                            "volume": 100,
                            "muted": true,
                            "playing": false,
                            "output": "out",
                            "streams": []
                        }
                    ]
                }
            })),
        ContributionSpec::new("tour", "step", "recording", "Panel", "Apps recording")
            .icon("mic")
            .order(42)
            .options(json!({
                "chapter": "panels",
                "since": "0.0.10",
                "caption": "Apps recording from the microphone get their own rows, with a volume and a mute each.",
                "payload": {
                    "connected": true,
                    "max_volume": 100,
                    "output": {
                        "name": "out",
                        "description": "Arctis Nova 7",
                        "volume": 62,
                        "muted": false,
                        "icon": "headset",
                        "default": true
                    },
                    "input": {
                        "name": "in",
                        "description": "Arctis Nova 7 microphone",
                        "volume": 80,
                        "muted": false,
                        "icon": "headset",
                        "default": true
                    },
                    "outputs": [],
                    "inputs": [],
                    "apps": [
                        {
                            "id": "Discord",
                            "target": "Discord",
                            "name": "Discord",
                            "title": "",
                            "icon": "discord",
                            "volume": 85,
                            "muted": false,
                            "playing": true,
                            "output": "out",
                            "streams": []
                        }
                    ],
                    "recorders": [
                        {
                            "id": "Discord",
                            "target": "recording:Discord",
                            "name": "Discord",
                            "title": "",
                            "icon": "discord",
                            "volume": 100,
                            "muted": false,
                            "recording": true,
                            "streams": []
                        },
                        {
                            "id": "OBS",
                            "target": "recording:OBS",
                            "name": "OBS",
                            "title": "",
                            "icon": "com.obsproject.Studio",
                            "volume": 70,
                            "muted": true,
                            "recording": false,
                            "streams": []
                        }
                    ]
                }
            })),
    ]
}
