/// The typed `org.scx.Loader` D-Bus surface — a pure, inspectable
/// declaration of the interface the candidate client implements, matching
/// the authority (`scx_loader 1.0.9` `src/dbus.rs` +
/// `src/main.rs`): `#[zbus::proxy(interface = "org.scx.Loader",
/// default_service = "org.scx.Loader", default_path =
/// "/org/scx/Loader")]`.
///
/// D-Bus method/property NAMES are the PascalCase forms zbus derives from
/// the snake_case Rust names (`zbus_macros 5.5.0 src/utils.rs::pascal_case`
/// — verified against the frozen zbus 5.5.0): `start_scheduler` →
/// `StartScheduler`, `current_scheduler` → `CurrentScheduler`, etc. The
/// wire signatures follow the zbus/zvariant encodings of the authority's
/// Rust types: `SupportedSched` (`#[zvariant(signature = "s")]`) → `"s"`;
/// `SchedMode` (fieldless enum, explicit discriminants, no `repr`) →
/// `"u"` (u32, zvariant's default for repr-less enums); `Vec<String>` →
/// `"as"`; `String` → `"s"`; `()` → no out args.
///
/// Courted by `scx/loader-interface` (non-VM source-derived + VM
/// `busctl introspect org.scx.Loader` witness).
use serde::{Deserialize, Serialize};

/// The D-Bus service name of scx_loader (also the interface name).
pub const SCX_LOADER_DBUS_NAME: &str = "org.scx.Loader";
/// The D-Bus object path.
pub const SCX_LOADER_DBUS_PATH: &str = "/org/scx/Loader";

/// One method of the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodDesc {
    pub name: String,
    /// `(arg_name, dbus_signature)` input arguments, in order.
    pub in_args: Vec<(String, String)>,
    /// Output argument signatures (empty for `()`).
    pub out_args: Vec<String>,
}

/// One property of the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDesc {
    pub name: String,
    pub signature: String,
    pub read: bool,
    pub write: bool,
}

/// The complete typed interface the client implements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDesc {
    pub interface: String,
    pub service: String,
    pub path: String,
    pub methods: Vec<MethodDesc>,
    pub properties: Vec<PropertyDesc>,
}

/// The candidate client's declared interface — the single source the zbus
/// proxy implementation and the `scx-introspect` witness are generated from.
pub fn loader_interface() -> InterfaceDesc {
    InterfaceDesc {
        interface: SCX_LOADER_DBUS_NAME.to_string(),
        service: SCX_LOADER_DBUS_NAME.to_string(),
        path: SCX_LOADER_DBUS_PATH.to_string(),
        methods: vec![
            MethodDesc {
                name: "StartScheduler".into(),
                in_args: vec![
                    ("scx_name".into(), "s".into()),
                    ("sched_mode".into(), "u".into()),
                ],
                out_args: vec![],
            },
            MethodDesc {
                name: "StartSchedulerWithArgs".into(),
                in_args: vec![
                    ("scx_name".into(), "s".into()),
                    ("scx_args".into(), "as".into()),
                ],
                out_args: vec![],
            },
            MethodDesc {
                name: "StopScheduler".into(),
                in_args: vec![],
                out_args: vec![],
            },
            MethodDesc {
                name: "SwitchScheduler".into(),
                in_args: vec![
                    ("scx_name".into(), "s".into()),
                    ("sched_mode".into(), "u".into()),
                ],
                out_args: vec![],
            },
            MethodDesc {
                name: "SwitchSchedulerWithArgs".into(),
                in_args: vec![
                    ("scx_name".into(), "s".into()),
                    ("scx_args".into(), "as".into()),
                ],
                out_args: vec![],
            },
        ],
        properties: vec![
            PropertyDesc {
                name: "CurrentScheduler".into(),
                signature: "s".into(),
                read: true,
                write: false,
            },
            PropertyDesc {
                name: "SchedulerMode".into(),
                signature: "u".into(),
                read: true,
                write: false,
            },
            PropertyDesc {
                name: "SupportedSchedulers".into(),
                signature: "as".into(),
                read: true,
                write: false,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_matches_authority_surface() {
        let iface = loader_interface();
        assert_eq!(iface.interface, "org.scx.Loader");
        assert_eq!(iface.service, "org.scx.Loader");
        assert_eq!(iface.path, "/org/scx/Loader");
        // zbus derives the D-Bus names via pascal_case (zbus_macros
        // 5.5.0 utils.rs) — the D-Bus surface is PascalCase
        let methods: Vec<&str> = iface.methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            methods,
            vec![
                "StartScheduler",
                "StartSchedulerWithArgs",
                "StopScheduler",
                "SwitchScheduler",
                "SwitchSchedulerWithArgs"
            ]
        );
        let props: Vec<&str> = iface.properties.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            props,
            vec!["CurrentScheduler", "SchedulerMode", "SupportedSchedulers"]
        );
        let switch = iface
            .methods
            .iter()
            .find(|m| m.name == "SwitchScheduler")
            .unwrap();
        assert_eq!(
            switch.in_args,
            vec![
                ("scx_name".to_string(), "s".to_string()),
                ("sched_mode".to_string(), "u".to_string())
            ]
        );
        let mode_prop = iface
            .properties
            .iter()
            .find(|p| p.name == "SchedulerMode")
            .unwrap();
        assert_eq!(mode_prop.signature, "u");
        assert!(mode_prop.read && !mode_prop.write);
    }
}
