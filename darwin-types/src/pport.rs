//! Push Port data - http://www.thalesgroup.com/rtti/PushPort/v12
use crate::forecasts::Ts;
use crate::schedule::{Schedule, DeactivatedSchedule};
use chrono::{DateTime, FixedOffset};
use std::io::Read;
use crate::errors::*;
use crate::deser::*;
use xml::reader::{XmlEvent, EventReader};

/// An Update or Snapshot Response element ('uR' or 'sR').
#[derive(Default, Clone, Debug)]
pub struct DataResponse {
    /// Train Status messages in this update.
    pub train_status: Vec<Ts>,
    /// Train Schedule messages in this update.
    pub schedule: Vec<Schedule>,
    /// Deactivated schedule notifications in this update.
    pub deactivated: Vec<DeactivatedSchedule>,
    /// A string describing the type of system that originated this update, e.g. "CIS" or "Darwin".
    pub update_origin: Option<String>,
    /// The source instance that generated this update, usually a CIS instance.
    pub request_source: Option<String>,
    /// The DCISRequestID value provided by the originator of this update. Used in conjunction with the requestSource attribute to ensure uniqueness.
    pub request_id: Option<String>
}
impl XmlDeserialize for DataResponse {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret: Self = Default::default();
        xml_attrs! { se, value,
            pat "updateOrigin" => {
                ret.update_origin = Some(value);
            },
            pat "requestSource" => {
                ret.request_source = Some(value);
            },
            pat "requestID" => {
                ret.request_id = Some(value);
            }
        }
        xml_iter! { se, reader,
            pragma lenient,
            parse "TS", Ts as ts {
                ret.train_status.push(ts);
            },
            parse "schedule", Schedule as sched {
                ret.schedule.push(sched);
            },
            parse "deactivated", DeactivatedSchedule as sched {
                ret.deactivated.push(sched);
            },
        }
        Ok(ret)
    }
}
/// An element found inside a 'Pport' object.
#[derive(Debug, Clone)]
pub enum PportElement {
    /// Update or Snapshot Response element ('uR' or 'sR').
    DataResponse(DataResponse),
    /// Some element that we don't yet process.
    Unimplemented
}
/// The type of element you get through the Darwin Push Port.
#[derive(Debug, Clone)]
pub struct Pport {
    /// The actual data.
    pub inner: PportElement,
    /// Local timestamp.
    pub ts: DateTime<FixedOffset>,
    /// Schema version.
    pub version: String
}
impl XmlDeserialize for Pport {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ts = None;
        let mut version = None;
        xml_attrs! { se, value,
            pat "ts" => {
                ts = Some(DateTime::parse_from_rfc3339(&value)?);
            },
            pat "version" => {
                version = Some(value);
            }
        }
        let mut elem = PportElement::Unimplemented;
        xml_iter! { se, reader,
            pragma lenient,
            parse "uR", DataResponse as dr {
                elem = PportElement::DataResponse(dr);
            },
            parse "sR", DataResponse as dr {
                elem = PportElement::DataResponse(dr);
            },
        }
        Ok(Self {
            inner: elem,
            ts: ts.ok_or(DarwinError::Missing("ts"))?,
            version: version.ok_or(DarwinError::Missing("version"))?,
        })
    }
}
/// Parse an XML document received from the push port.
pub fn parse_pport_document<R: Read>(r: R) -> Result<Pport> {
    let mut reader = EventReader::new(r);
    loop {
        match reader.next()? {
            x @ XmlEvent::StartElement { .. } => {
                let xse = XmlStartElement::from_evt(x);
                if xse.name.local_name == "Pport" {
                    return Ok(Pport::from_xml_iter(xse, &mut reader)?);
                }
                else {
                    Err(DarwinError::UnexpectedStart(xse.name))?;
                }
            },
            XmlEvent::EndDocument { .. } => {
                Err(DarwinError::UnexpectedEnd)?
            },
            _ => {}
        }
    }
}
