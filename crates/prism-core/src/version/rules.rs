//! Launch rules and runtime context — ports of `minecraft/Rule.cpp` and
//! `RuntimeContext.h`.
//!
//! Current Prism rules carry only an action (`allow`/`disallow`) and an
//! optional OS descriptor; feature keys that appear in Mojang data are not
//! modeled by Prism's `Rule` (its `fromJson` ignores them) and are ignored
//! here too, deliberately.

use serde_json::Value;

/// Outcome of evaluating a rule against a runtime context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The rule does not decide; keep evaluating.
    Defer,
    /// The rule allows.
    Allow,
    /// The rule disallows.
    Disallow,
}

/// Rule action from JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `allow`.
    Allow,
    /// `disallow`.
    Disallow,
}

/// Optional OS restriction of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OsSpec {
    /// OS name: `windows`, `linux` or `osx`.
    pub name: String,
    /// OS version filter (rarely set, kept for round-tripping).
    pub version: String,
}

/// One launch rule.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rule {
    /// Action applied when the rule matches.
    pub action: Option<Action>,
    /// OS restriction, when present.
    pub os: Option<OsSpec>,
}

impl Rule {
    /// Parse from a rule JSON object (`Rule::fromJson`). Unknown actions
    /// leave the action unset (defaulting to allow on apply, matching the
    /// C++ default-initialized enum).
    pub fn from_json(value: &Value) -> Rule {
        let obj = match value.as_object() {
            Some(o) => o,
            None => return Rule::default(),
        };
        let action = match obj.get("action").and_then(|v| v.as_str()) {
            Some("allow") => Some(Action::Allow),
            Some("disallow") => Some(Action::Disallow),
            _ => None,
        };
        let os = obj.get("os").and_then(|v| v.as_object()).and_then(|os| {
            let name = match os.get("name") {
                Some(Value::String(s)) => s.clone(),
                _ => return None, // non-string name -> no OS spec (toString() -> null)
            };
            let version = os.get("version").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            Some(OsSpec { name, version })
        });
        Rule { action, os }
    }

    /// Serialize (`Rule::toJson`): `action` + optional `os{name, version?}`.
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        match self.action {
            Some(Action::Allow) => {
                obj.insert("action".into(), Value::String("allow".into()));
            }
            Some(Action::Disallow) => {
                obj.insert("action".into(), Value::String("disallow".into()));
            }
            None => {}
        }
        if let Some(os) = &self.os {
            let mut os_obj = serde_json::Map::new();
            os_obj.insert("name".into(), Value::String(os.name.clone()));
            if !os.version.is_empty() {
                os_obj.insert("version".into(), Value::String(os.version.clone()));
            }
            obj.insert("os".into(), Value::Object(os_obj));
        }
        Value::Object(obj)
    }

    /// Evaluate (`Rule::apply`): Defer when the OS does not match, else the
    /// action (unset actions read as allow).
    pub fn apply(&self, ctx: &RuntimeContext) -> Applied {
        if let Some(os) = &self.os {
            if !ctx.classifier_matches(&os.name) {
                return Applied::Defer;
            }
        }
        match self.action {
            Some(Action::Allow) | None => Applied::Allow,
            Some(Action::Disallow) => Applied::Disallow,
        }
    }
}

/// Runtime environment for rule evaluation (`RuntimeContext`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeContext {
    /// Java-reported architecture word size ("32"/"64"/...).
    pub java_architecture: String,
    /// Java-reported raw architecture (os.arch), e.g. `amd64`.
    pub java_real_architecture: String,
    /// Launcher system: `windows`, `linux` or `osx`.
    pub system: String,
}

impl RuntimeContext {
    /// Build the context that a plain (non-instance-settings) launch would
    /// use on the current host.
    pub fn current_host() -> RuntimeContext {
        RuntimeContext {
            java_architecture: if cfg!(target_pointer_width = "64") { "64".into() } else { "32".into() },
            java_real_architecture: std::env::consts::ARCH.to_string(),
            system: match crate::paths::System::current() {
                crate::paths::System::Windows => "windows".into(),
                crate::paths::System::MacOS => "osx".into(),
                crate::paths::System::Linux => "linux".into(),
            },
        }
    }

    /// Normalize os.arch spellings (`mappedJavaRealArchitecture`).
    pub fn mapped_arch(&self) -> &str {
        match self.java_real_architecture.as_str() {
            "amd64" => "x86_64",
            "i386" | "i686" => "x86",
            "aarch64" => "arm64",
            "arm" | "armhf" => "arm32",
            other => other,
        }
    }

    /// Precise native classifier: `<system>-<mapped arch>`
    /// (`RuntimeContext::getClassifier`).
    pub fn classifier(&self) -> String {
        format!("{}-{}", self.system, self.mapped_arch())
    }

    /// `isLegacyArch`: x86_64 / x86 — the architectures Mojang's older
    /// natives were implicitly built for.
    pub fn is_legacy_arch(&self) -> bool {
        matches!(self.mapped_arch(), "x86_64" | "x86")
    }

    /// `classifierMatches`: precise `<system>-<arch>` match, or bare
    /// `<system>` match on legacy architectures.
    pub fn classifier_matches(&self, target: &str) -> bool {
        if target == self.classifier() {
            return true;
        }
        self.is_legacy_arch() && target == self.system
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(system: &str, arch: &str, java_arch: &str) -> RuntimeContext {
        RuntimeContext {
            system: system.into(),
            java_real_architecture: arch.into(),
            java_architecture: java_arch.into(),
        }
    }

    #[test]
    fn rule_json_round_trip() {
        let r = Rule::from_json(&json!({"action": "allow", "os": {"name": "osx"}}));
        assert_eq!(r.action, Some(Action::Allow));
        assert_eq!(r.os.as_ref().unwrap().name, "osx");
        assert_eq!(r.to_json(), json!({"action": "allow", "os": {"name": "osx"}}));

        let r = Rule::from_json(&json!({"action": "disallow", "os": {"name": "windows", "version": "10"}}));
        assert_eq!(r.to_json(), json!({"action": "disallow", "os": {"name": "windows", "version": "10"}}));
    }

    #[test]
    fn rule_without_matching_os_defers() {
        let r = Rule::from_json(&json!({"action": "allow", "os": {"name": "osx"}}));
        assert_eq!(r.apply(&ctx("linux", "x86_64", "64")), Applied::Defer);
        assert_eq!(r.apply(&ctx("osx", "x86_64", "64")), Applied::Allow);
        // bare os name matches only on legacy arch
        assert_eq!(r.apply(&ctx("osx", "arm64", "64")), Applied::Defer);
    }

    #[test]
    fn feature_keys_are_ignored_like_prism() {
        let r = Rule::from_json(&json!({"action": "allow", "features": {"is_demo_user": true}}));
        assert_eq!(r.os, None);
        assert_eq!(r.apply(&ctx("linux", "x86_64", "64")), Applied::Allow);
    }

    #[test]
    fn unknown_action_defaults_to_allow() {
        let r = Rule::from_json(&json!({"action": "wat"}));
        assert_eq!(r.action, None);
        assert_eq!(r.apply(&ctx("linux", "x86_64", "64")), Applied::Allow);
        let r = Rule::from_json(&json!({"action": null}));
        assert_eq!(r.apply(&ctx("linux", "x86_64", "64")), Applied::Allow);
    }

    #[test]
    fn classifier_mapping_and_legacy_arch() {
        assert_eq!(ctx("windows", "amd64", "64").classifier(), "windows-x86_64");
        assert_eq!(ctx("linux", "i686", "32").classifier(), "linux-x86");
        assert_eq!(ctx("osx", "aarch64", "64").classifier(), "osx-arm64");
        assert_eq!(ctx("linux", "armhf", "32").classifier(), "linux-arm32");
        assert!(ctx("linux", "x86_64", "64").is_legacy_arch());
        assert!(!ctx("linux", "arm64", "64").is_legacy_arch());
        // precise match works everywhere; bare system only on legacy arch
        let c = ctx("linux", "arm64", "64");
        assert!(c.classifier_matches("linux-arm64"));
        assert!(!c.classifier_matches("linux"));
        let l = ctx("linux", "x86_64", "64");
        assert!(l.classifier_matches("linux"));
    }

    #[test]
    fn current_host_yields_sane_system() {
        let c = RuntimeContext::current_host();
        assert!(matches!(c.system.as_str(), "windows" | "linux" | "osx"));
    }
}
