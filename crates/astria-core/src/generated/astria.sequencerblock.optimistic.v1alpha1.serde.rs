impl serde::Serialize for GetFinalizedBlockInfoStreamRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.address.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamRequest", len)?;
        if let Some(v) = self.address.as_ref() {
            struct_ser.serialize_field("address", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetFinalizedBlockInfoStreamRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "address",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Address,
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
                            "address" => Ok(GeneratedField::Address),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetFinalizedBlockInfoStreamRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetFinalizedBlockInfoStreamRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut address__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Address => {
                            if address__.is_some() {
                                return Err(serde::de::Error::duplicate_field("address"));
                            }
                            address__ = map_.next_value()?;
                        }
                    }
                }
                Ok(GetFinalizedBlockInfoStreamRequest {
                    address: address__,
                })
            }
        }
        deserializer.deserialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for GetFinalizedBlockInfoStreamResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.block_info.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamResponse", len)?;
        if let Some(v) = self.block_info.as_ref() {
            struct_ser.serialize_field("blockInfo", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetFinalizedBlockInfoStreamResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "block_info",
            "blockInfo",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            BlockInfo,
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
                            "blockInfo" | "block_info" => Ok(GeneratedField::BlockInfo),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetFinalizedBlockInfoStreamResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetFinalizedBlockInfoStreamResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut block_info__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::BlockInfo => {
                            if block_info__.is_some() {
                                return Err(serde::de::Error::duplicate_field("blockInfo"));
                            }
                            block_info__ = map_.next_value()?;
                        }
                    }
                }
                Ok(GetFinalizedBlockInfoStreamResponse {
                    block_info: block_info__,
                })
            }
        }
        deserializer.deserialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetFinalizedBlockInfoStreamResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for GetOptimisticBlockStreamRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.rollup_id.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamRequest", len)?;
        if let Some(v) = self.rollup_id.as_ref() {
            struct_ser.serialize_field("rollupId", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetOptimisticBlockStreamRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "rollup_id",
            "rollupId",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            RollupId,
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
                            "rollupId" | "rollup_id" => Ok(GeneratedField::RollupId),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetOptimisticBlockStreamRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetOptimisticBlockStreamRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut rollup_id__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::RollupId => {
                            if rollup_id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("rollupId"));
                            }
                            rollup_id__ = map_.next_value()?;
                        }
                    }
                }
                Ok(GetOptimisticBlockStreamRequest {
                    rollup_id: rollup_id__,
                })
            }
        }
        deserializer.deserialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for GetOptimisticBlockStreamResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.block.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamResponse", len)?;
        if let Some(v) = self.block.as_ref() {
            struct_ser.serialize_field("block", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for GetOptimisticBlockStreamResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "block",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Block,
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
                            "block" => Ok(GeneratedField::Block),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = GetOptimisticBlockStreamResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<GetOptimisticBlockStreamResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut block__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Block => {
                            if block__.is_some() {
                                return Err(serde::de::Error::duplicate_field("block"));
                            }
                            block__ = map_.next_value()?;
                        }
                    }
                }
                Ok(GetOptimisticBlockStreamResponse {
                    block: block__,
                })
            }
        }
        deserializer.deserialize_struct("astria.sequencerblock.optimistic.v1alpha1.GetOptimisticBlockStreamResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for SequencerFinalizedBlockInfo {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.height != 0 {
            len += 1;
        }
        if !self.block_hash.is_empty() {
            len += 1;
        }
        if self.pending_nonce != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("astria.sequencerblock.optimistic.v1alpha1.SequencerFinalizedBlockInfo", len)?;
        if self.height != 0 {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("height", ToString::to_string(&self.height).as_str())?;
        }
        if !self.block_hash.is_empty() {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("blockHash", pbjson::private::base64::encode(&self.block_hash).as_str())?;
        }
        if self.pending_nonce != 0 {
            struct_ser.serialize_field("pendingNonce", &self.pending_nonce)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for SequencerFinalizedBlockInfo {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "height",
            "block_hash",
            "blockHash",
            "pending_nonce",
            "pendingNonce",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Height,
            BlockHash,
            PendingNonce,
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
                            "height" => Ok(GeneratedField::Height),
                            "blockHash" | "block_hash" => Ok(GeneratedField::BlockHash),
                            "pendingNonce" | "pending_nonce" => Ok(GeneratedField::PendingNonce),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = SequencerFinalizedBlockInfo;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct astria.sequencerblock.optimistic.v1alpha1.SequencerFinalizedBlockInfo")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<SequencerFinalizedBlockInfo, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut height__ = None;
                let mut block_hash__ = None;
                let mut pending_nonce__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Height => {
                            if height__.is_some() {
                                return Err(serde::de::Error::duplicate_field("height"));
                            }
                            height__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::BlockHash => {
                            if block_hash__.is_some() {
                                return Err(serde::de::Error::duplicate_field("blockHash"));
                            }
                            block_hash__ = 
                                Some(map_.next_value::<::pbjson::private::BytesDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::PendingNonce => {
                            if pending_nonce__.is_some() {
                                return Err(serde::de::Error::duplicate_field("pendingNonce"));
                            }
                            pending_nonce__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                    }
                }
                Ok(SequencerFinalizedBlockInfo {
                    height: height__.unwrap_or_default(),
                    block_hash: block_hash__.unwrap_or_default(),
                    pending_nonce: pending_nonce__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("astria.sequencerblock.optimistic.v1alpha1.SequencerFinalizedBlockInfo", FIELDS, GeneratedVisitor)
    }
}
