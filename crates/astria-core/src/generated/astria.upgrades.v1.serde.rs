impl serde::Serialize for BaseInfo {
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
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.BaseInfo", len)?;
        if self.activation_height != 0 {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("activationHeight", ToString::to_string(&self.activation_height).as_str())?;
        }
        if self.shutdown_required {
            struct_ser.serialize_field("shutdownRequired", &self.shutdown_required)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for BaseInfo {
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
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            ActivationHeight,
            ShutdownRequired,
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
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = BaseInfo;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.BaseInfo")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<BaseInfo, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut activation_height__ = None;
                let mut shutdown_required__ = None;
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
                    }
                }
                Ok(BaseInfo {
                    activation_height: activation_height__.unwrap_or_default(),
                    shutdown_required: shutdown_required__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.BaseInfo", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ConnectOracleUpgrade {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.base_info.is_some() {
            len += 1;
        }
        if self.genesis.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.ConnectOracleUpgrade", len)?;
        if let Some(v) = self.base_info.as_ref() {
            struct_ser.serialize_field("baseInfo", v)?;
        }
        if let Some(v) = self.genesis.as_ref() {
            struct_ser.serialize_field("genesis", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ConnectOracleUpgrade {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "base_info",
            "baseInfo",
            "genesis",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            BaseInfo,
            Genesis,
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
                            "baseInfo" | "base_info" => Ok(GeneratedField::BaseInfo),
                            "genesis" => Ok(GeneratedField::Genesis),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ConnectOracleUpgrade;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.ConnectOracleUpgrade")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ConnectOracleUpgrade, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut base_info__ = None;
                let mut genesis__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::BaseInfo => {
                            if base_info__.is_some() {
                                return Err(serde::de::Error::duplicate_field("baseInfo"));
                            }
                            base_info__ = map_.next_value()?;
                        }
                        GeneratedField::Genesis => {
                            if genesis__.is_some() {
                                return Err(serde::de::Error::duplicate_field("genesis"));
                            }
                            genesis__ = map_.next_value()?;
                        }
                    }
                }
                Ok(ConnectOracleUpgrade {
                    base_info: base_info__,
                    genesis: genesis__,
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.ConnectOracleUpgrade", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Upgrades {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.connect_oracle.is_some() {
            len += 1;
        }
        if self.whatever.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.Upgrades", len)?;
        if let Some(v) = self.connect_oracle.as_ref() {
            struct_ser.serialize_field("connectOracle", v)?;
        }
        if let Some(v) = self.whatever.as_ref() {
            struct_ser.serialize_field("whatever", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Upgrades {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "connect_oracle",
            "connectOracle",
            "whatever",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            ConnectOracle,
            Whatever,
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
                            "connectOracle" | "connect_oracle" => Ok(GeneratedField::ConnectOracle),
                            "whatever" => Ok(GeneratedField::Whatever),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Upgrades;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.Upgrades")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Upgrades, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut connect_oracle__ = None;
                let mut whatever__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::ConnectOracle => {
                            if connect_oracle__.is_some() {
                                return Err(serde::de::Error::duplicate_field("connectOracle"));
                            }
                            connect_oracle__ = map_.next_value()?;
                        }
                        GeneratedField::Whatever => {
                            if whatever__.is_some() {
                                return Err(serde::de::Error::duplicate_field("whatever"));
                            }
                            whatever__ = map_.next_value()?;
                        }
                    }
                }
                Ok(Upgrades {
                    connect_oracle: connect_oracle__,
                    whatever: whatever__,
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.Upgrades", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for WhateverUpgrade {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.base_info.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.upgrades.v1.WhateverUpgrade", len)?;
        if let Some(v) = self.base_info.as_ref() {
            struct_ser.serialize_field("baseInfo", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for WhateverUpgrade {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "base_info",
            "baseInfo",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            BaseInfo,
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
                            "baseInfo" | "base_info" => Ok(GeneratedField::BaseInfo),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = WhateverUpgrade;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.upgrades.v1.WhateverUpgrade")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<WhateverUpgrade, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut base_info__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::BaseInfo => {
                            if base_info__.is_some() {
                                return Err(serde::de::Error::duplicate_field("baseInfo"));
                            }
                            base_info__ = map_.next_value()?;
                        }
                    }
                }
                Ok(WhateverUpgrade {
                    base_info: base_info__,
                })
            }
        }
        deserializer.deserialize_struct("astria.upgrades.v1.WhateverUpgrade", FIELDS, GeneratedVisitor)
    }
}
