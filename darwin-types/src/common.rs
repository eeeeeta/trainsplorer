//! Common types - http://www.thalesgroup.com/rtti/PushPort/CommonTypes/v1
use errors::*;
use deser::*;
use std::io::Read;
use xml::reader::{XmlEvent, EventReader};
use chrono::NaiveTime;

/// A scheduled time used to distinguish a location on circular routes.
///
/// Note that all scheduled time attributes are marked as optional, but at
/// least one must always be supplied. Only one value is required, and typically
/// this should be the wtd value. However, for locations that have no wtd, or
/// for clients that deal exclusively with public times, another value that
/// is valid for the location may be supplied.
#[derive(Builder, Default, Clone, Debug)]
#[builder(default, build_fn(validate = "Self::validate"))]
pub struct CircularTimes {
    /// Working time of arrival.
    pub wta: Option<NaiveTime>,
    /// Working time of departure.
    pub wtd: Option<NaiveTime>,
    /// Working time of pass.
    pub wtp: Option<NaiveTime>,
    /// Public time of arrival.
    pub pta: Option<NaiveTime>,
    /// Public time of departure.
    pub ptd: Option<NaiveTime>,
}
impl CircularTimesBuilder {
    fn validate(&self) -> ::std::result::Result<(), String> {
        if self.wta.is_some() 
            || self.wtd.is_some()
            || self.wtp.is_some()
            || self.pta.is_some()
            || self.ptd.is_some() {
            Ok(())
        }
        else {
            Err("At least one CircularTimes attribute must be defined".into())
        }
    }
}
/// Type used to represent a cancellation or late running reason.
#[derive(Builder, Default, Clone, Debug)]
#[builder(private)]
pub struct DisruptionReason {
    /// A Darwin Reason Code.
    pub reason: String,
    /// Optional TIPLOC where the reason refers to, e.g. "signalling failure at Cheadle Hulme".
    #[builder(default)]
    pub tiploc: Option<String>,
    /// If true, the tiploc attribute should be interpreted as "near", e.g. "signalling failure near Cheadle Hulme".
    #[builder(default)]
    pub near: bool
}
impl XmlDeserialize for DisruptionReason {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret = DisruptionReasonBuilder::default();
        xml_attrs! { se, value,
            parse near on ret,
            with tiploc on ret {
                Some(value)
            },
        }
        xml_iter! { se, reader,
            pat XmlEvent::Characters(data) => {
                ret.reason(data);
            }
        }
        xml_build!(ret);
        Ok(ret)
    }
}
