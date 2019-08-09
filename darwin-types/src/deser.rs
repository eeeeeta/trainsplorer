//! Abstractions for XML deserialization.
use xml::name::OwnedName;
use xml::attribute::OwnedAttribute;
use xml::namespace::Namespace;
use xml::reader::{EventReader, XmlEvent};
use crate::errors::Result;
use std::io::Read;

pub struct XmlStartElement {
    pub name: OwnedName,
    pub attributes: Vec<OwnedAttribute>,
    pub namespace: Namespace
}
impl XmlStartElement {
    pub fn from_evt(e: XmlEvent) -> Self {
        match e {
            XmlEvent::StartElement { name, attributes, namespace } => {
                Self { name, attributes, namespace }
            },
            other => {
                panic!("tried to convert {:?} into XmlStartElement", other)
            }
        }
    }
}
pub trait XmlDeserialize: Sized {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self>;
}
