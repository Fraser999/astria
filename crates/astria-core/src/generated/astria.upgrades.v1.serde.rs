impl serde::Serialize for Change {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.description.is_empty() {
            len += 1;
        }
        if self.activation_height.is_some() {
            len += 1;
        }
        if self.value.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.Change", len)?;
        if !self.description.is_empty() {
            struct_ser.serialize_field("description", &self.description)?;
        }
        if let Some(v) = self.activation_height.as_ref() {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("activationHeight", ToString::to_string(&v).as_str())?;
        }
        if let Some(v) = self.value.as_ref() {
            struct_ser.serialize_field("value", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Change {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "description",
            "activation_height",
            "activationHeight",
            "value",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Description,
            ActivationHeight,
            Value,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "description" => Ok(GeneratedField::Description),
                            "activationHeight" | "activation_height" => Ok(GeneratedField::ActivationHeight),
                            "value" => Ok(GeneratedField::Value),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Change;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.Change")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Change, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut description__ = None;
                let mut activation_height__ = None;
                let mut value__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Description => {
                            if description__.is_some() {
                                return Err(serde::de::Error::duplicate_field("description"));
                            }
                            description__ = Some(map_.next_value()?);
                        }
                        GeneratedField::ActivationHeight => {
                            if activation_height__.is_some() {
                                return Err(serde::de::Error::duplicate_field("activationHeight"));
                            }
                            activation_height__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Value => {
                            if value__.is_some() {
                                return Err(serde::de::Error::duplicate_field("value"));
                            }
                            value__ = map_.next_value()?;
                        }
                    }
                }
                Ok(Change {
                    description: description__.unwrap_or_default(),
                    activation_height: activation_height__,
                    value: value__,
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.Change", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Upgrade {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.activation_height != 0 {
            len += 1;
        }
        if self.shutdown_required {
            len += 1;
        }
        if !self.changes.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.Upgrade", len)?;
        if self.activation_height != 0 {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("activationHeight", ToString::to_string(&self.activation_height).as_str())?;
        }
        if self.shutdown_required {
            struct_ser.serialize_field("shutdownRequired", &self.shutdown_required)?;
        }
        if !self.changes.is_empty() {
            struct_ser.serialize_field("changes", &self.changes)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Upgrade {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "activation_height",
            "activationHeight",
            "shutdown_required",
            "shutdownRequired",
            "changes",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            ActivationHeight,
            ShutdownRequired,
            Changes,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "activationHeight" | "activation_height" => Ok(GeneratedField::ActivationHeight),
                            "shutdownRequired" | "shutdown_required" => Ok(GeneratedField::ShutdownRequired),
                            "changes" => Ok(GeneratedField::Changes),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Upgrade;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.Upgrade")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Upgrade, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut activation_height__ = None;
                let mut shutdown_required__ = None;
                let mut changes__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::ActivationHeight => {
                            if activation_height__.is_some() {
                                return Err(serde::de::Error::duplicate_field("activationHeight"));
                            }
                            activation_height__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::ShutdownRequired => {
                            if shutdown_required__.is_some() {
                                return Err(serde::de::Error::duplicate_field("shutdownRequired"));
                            }
                            shutdown_required__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Changes => {
                            if changes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("changes"));
                            }
                            changes__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(Upgrade {
                    activation_height: activation_height__.unwrap_or_default(),
                    shutdown_required: shutdown_required__.unwrap_or_default(),
                    changes: changes__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.Upgrade", FIELDS, GeneratedVisitor)
    }
}
