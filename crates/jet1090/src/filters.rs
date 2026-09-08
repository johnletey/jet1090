use rs1090::decode::{TimedMessage, DF, ICAO};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Filters {
    pub df_filter: Option<Vec<u16>>,
    pub aircraft_filter: Option<Vec<ICAO>>,
    //pub sensor_filter: Option<Vec<String>>,
}

/// The two fields a filter looks at, extracted once per message so that
/// several filters (the global one, then one per /stream client) can be
/// applied without walking the decoded message again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterKey {
    pub df: u16,
    /// DF19 carries no aircraft address
    pub icao24: Option<ICAO>,
}

impl Filters {
    /// None when the message could not be decoded
    pub fn key(msg: &TimedMessage) -> Option<FilterKey> {
        let msg = msg.message.as_ref()?;
        let (df, icao24) = match &msg.df {
            DF::ShortAirAirSurveillance { ap, .. } => (0, Some((*ap).into())),
            DF::SurveillanceAltitudeReply { ap, .. } => (4, Some((*ap).into())),
            DF::SurveillanceIdentityReply { ap, .. } => (5, Some((*ap).into())),
            DF::AllCallReply { icao, .. } => (11, Some(*icao)),
            DF::LongAirAirSurveillance { ap, .. } => (16, Some((*ap).into())),
            DF::ExtendedSquitterADSB(adsb) => (17, Some(adsb.icao24)),
            DF::ExtendedSquitterTisB { cf, .. } => (18, Some(cf.aa)),
            DF::ExtendedSquitterMilitary { .. } => (19, None),
            DF::CommBAltitudeReply { ap, .. } => (20, Some((*ap).into())),
            DF::CommBIdentityReply { ap, .. } => (21, Some((*ap).into())),
            DF::CommDExtended { parity, .. } => (24, Some(*parity)),
        };
        Some(FilterKey { df, icao24 })
    }

    /// An absent or empty list accepts everything, as the CLI options do
    pub fn matches(&self, key: &FilterKey) -> bool {
        let aircraft_ok = match &self.aircraft_filter {
            Some(list) if !list.is_empty() => {
                key.icao24.is_some_and(|icao24| list.contains(&icao24))
            }
            _ => true,
        };
        let df_ok = match &self.df_filter {
            Some(list) if !list.is_empty() => list.contains(&key.df),
            _ => true,
        };
        aircraft_ok && df_ok
    }

    #[cfg(test)]
    pub fn is_in(&self, msg: &TimedMessage) -> bool {
        Self::key(msg).is_some_and(|key| self.matches(&key))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rs1090::decode::Message;

    #[test]
    fn test_filter() {
        let mut tmsg = TimedMessage {
            timestamp: 0.,
            frame: hex::decode("8c4841753a9a153237aef0f275be").unwrap(),
            message: None,
            metadata: vec![],
            decode_time: None,
        };
        tmsg.message = Message::try_from(tmsg.frame.as_slice()).ok();

        let toml_data = r#"
        df_filter = []
        aircraft_filter = []
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(Filters::is_in(&filter, &tmsg));

        let toml_data = r#"
            df_filter = [17, 20, 21]
            aircraft_filter = []
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(Filters::is_in(&filter, &tmsg));

        let toml_data = r#"
            df_filter = [17, 20, 21]
            aircraft_filter = ["484175"]
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(Filters::is_in(&filter, &tmsg));

        let toml_data = r#"
            df_filter = [11]
            aircraft_filter = ["484175"]
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(!Filters::is_in(&filter, &tmsg));

        let toml_data = r#"
            df_filter = [17, 20, 21]
            aircraft_filter = ["333333"]
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(!Filters::is_in(&filter, &tmsg));

        let mut tmsg = TimedMessage {
            timestamp: 1735943148.353877,
            frame: hex::decode("02c18c3b323e4f").unwrap(),
            message: None,
            metadata: vec![],
            decode_time: None,
        };
        tmsg.message = Message::try_from(tmsg.frame.as_slice()).ok();

        let toml_data = r#"
            df_filter = [17, 20, 21]
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(!Filters::is_in(&filter, &tmsg));

        let toml_data = r#"
            df_filter = [0]
        "#;
        let filter: Filters =
            toml::from_str(toml_data).expect("Failed to deserialize TOML");

        assert!(Filters::is_in(&filter, &tmsg));
    }

    fn decoded(frame: &str) -> TimedMessage {
        let frame = hex::decode(frame).unwrap();
        TimedMessage {
            timestamp: 0.,
            message: Message::try_from(frame.as_slice()).ok(),
            frame,
            metadata: vec![],
            decode_time: None,
        }
    }

    #[test]
    fn tisb_messages_filter_on_the_announced_address() {
        let tmsg = decoded("95c639eefbffffedd5fefbff4f6f");
        assert_eq!(
            Filters::key(&tmsg),
            Some(FilterKey {
                df: 18,
                icao24: Some(ICAO(0xc639ee))
            })
        );
        let filter = Filters {
            df_filter: None,
            aircraft_filter: Some(vec![ICAO(0xc639ee)]),
        };
        assert!(filter.is_in(&tmsg));
    }

    #[test]
    fn military_messages_have_no_address() {
        let tmsg = decoded("9800000000000000000000000000");
        let key = Filters::key(&tmsg).unwrap();
        assert_eq!(key.df, 19);
        assert_eq!(key.icao24, None);

        let anyone = Filters {
            df_filter: Some(vec![19]),
            aircraft_filter: None,
        };
        assert!(anyone.is_in(&tmsg));

        let one_aircraft = Filters {
            df_filter: None,
            aircraft_filter: Some(vec![ICAO(0x484175)]),
        };
        assert!(!one_aircraft.is_in(&tmsg));
    }
}
