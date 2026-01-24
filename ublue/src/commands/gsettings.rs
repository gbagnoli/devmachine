use crate::cli::Format;
use crate::errors::AppError;
use gio::glib;
use gsettings_macro::gen_settings;
use serde::Serialize;
use std::fmt::Write;

#[gen_settings(
    file = "./gschemas/org.gnome.settings-daemon.plugins.power.gschema.xml",
    id = "org.gnome.settings-daemon.plugins.power"
)]
pub struct PowerSettings;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.power";
const SETTINGS: &[(&str, &str)] = &[
    ("power-button-action", "interactive"),
    ("power-saver-profile-on-low-battery", "true"),
    ("sleep-inactive-ac-timeout", "0"),
    ("sleep-inactive-ac-type", "nothing"),
    ("sleep-inactive-battery-timeout", "900"),
    ("sleep-inactive-battery-type", "suspend"),
];

#[derive(Serialize)]
struct SettingEntry {
    key: String,
    target_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated: Option<bool>,
}

fn get_setting_value(settings: &PowerSettings, key: &str) -> Result<String, AppError> {
    match key {
        "power-button-action" => Ok(settings.power_button_action()),
        "power-saver-profile-on-low-battery" => {
            Ok(settings.power_saver_profile_on_low_battery().to_string())
        }
        "sleep-inactive-ac-timeout" => Ok(settings.sleep_inactive_ac_timeout().to_string()),
        "sleep-inactive-ac-type" => Ok(settings.sleep_inactive_ac_type()),
        "sleep-inactive-battery-timeout" => {
            Ok(settings.sleep_inactive_battery_timeout().to_string())
        }
        "sleep-inactive-battery-type" => Ok(settings.sleep_inactive_battery_type()),
        _ => Err(AppError::Custom(format!("Unknown key: {}", key))),
    }
}

fn set_setting_value(settings: &PowerSettings, key: &str, target: &str) -> Result<(), AppError> {
    match key {
        "power-button-action" => {
            settings.set_power_button_action(target);
            Ok(())
        }
        "power-saver-profile-on-low-battery" => {
            let val = target
                .parse::<bool>()
                .map_err(|_| AppError::Custom(format!("Invalid bool value: {}", target)))?;
            settings.set_power_saver_profile_on_low_battery(val);
            Ok(())
        }
        "sleep-inactive-ac-timeout" => {
            let val = target
                .parse::<i32>()
                .map_err(|_| AppError::Custom(format!("Invalid int value: {}", target)))?;
            settings.set_sleep_inactive_ac_timeout(val);
            Ok(())
        }
        "sleep-inactive-ac-type" => {
            settings.set_sleep_inactive_ac_type(target);
            Ok(())
        }
        "sleep-inactive-battery-timeout" => {
            let val = target
                .parse::<i32>()
                .map_err(|_| AppError::Custom(format!("Invalid int value: {}", target)))?;
            settings.set_sleep_inactive_battery_timeout(val);
            Ok(())
        }
        "sleep-inactive-battery-type" => {
            settings.set_sleep_inactive_battery_type(target);
            Ok(())
        }
        _ => Err(AppError::Custom(format!("Unknown key: {}", key))),
    }
}

fn format_output(entries: &[SettingEntry], format: Format) -> Result<String, AppError> {
    match format {
        Format::Human => {
            let mut output = String::new();
            writeln!(output, "Schema: {}", SCHEMA)?;
            for entry in entries {
                write!(output, "  {}: target={}", entry.key, entry.target_value)?;
                if let Some(current) = &entry.current_value {
                    write!(output, ", current={}", current)?;
                }
                if let Some(updated) = entry.updated {
                    if updated {
                        writeln!(output, " [updated]")?;
                    } else {
                        writeln!(output, " [unchanged]")?;
                    }
                } else {
                    writeln!(output)?;
                }
            }
            Ok(output)
        }
        Format::Json => Ok(serde_json::to_string_pretty(entries)
            .map_err(|e| AppError::Custom(e.to_string()))?),
        Format::Csv => {
            let mut output = String::from("key,target_value,current_value,updated\n");
            for entry in entries {
                writeln!(
                    output,
                    "{},{},{},{}",
                    entry.key,
                    entry.target_value,
                    entry.current_value.as_deref().unwrap_or(""),
                    entry.updated.map(|b| b.to_string()).unwrap_or_default()
                )?;
            }
            Ok(output)
        }
        Format::Tsv => {
            let mut output = String::from("key\ttarget_value\tcurrent_value\tupdated\n");
            for entry in entries {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    entry.key,
                    entry.target_value,
                    entry.current_value.as_deref().unwrap_or(""),
                    entry.updated.map(|b| b.to_string()).unwrap_or_default()
                )?;
            }
            Ok(output)
        }
    }
}

pub fn list(format: Format) -> Result<String, AppError> {
    let entries: Vec<SettingEntry> = SETTINGS
        .iter()
        .map(|(key, target)| SettingEntry {
            key: key.to_string(),
            target_value: target.to_string(),
            current_value: None,
            updated: None,
        })
        .collect();

    format_output(&entries, format)
}

pub fn show(format: Format) -> Result<String, AppError> {
    let settings = PowerSettings::default();
    let mut entries = Vec::new();
    for (key, target) in SETTINGS {
        let current = match get_setting_value(&settings, key) {
            Ok(val) => val,
            Err(e) => format!("<Error: {}>", e),
        };
        entries.push(SettingEntry {
            key: key.to_string(),
            target_value: target.to_string(),
            current_value: Some(current),
            updated: None,
        });
    }

    format_output(&entries, format)
}

pub fn set(format: Format) -> Result<String, AppError> {
    let settings = PowerSettings::default();
    let mut entries = Vec::new();

    for (key, target) in SETTINGS {
        let current_val =
            get_setting_value(&settings, key).unwrap_or_else(|e| format!("<Error: {}>", e));

        let mut updated = false;

        if current_val != *target {
            if let Err(e) = set_setting_value(&settings, key, target) {
                return Err(AppError::Custom(format!(
                    "Failed to update {} {} to {}: {}",
                    SCHEMA, key, target, e
                )));
            }
            updated = true;
        }

        entries.push(SettingEntry {
            key: key.to_string(),
            target_value: target.to_string(),
            current_value: Some(current_val),
            updated: Some(updated),
        });
    }

    format_output(&entries, format)
}
