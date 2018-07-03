use chrono::NaiveTime;
use errors::{DarwinError, Result};
use std::str::FromStr;
use deser::*;
use std::io::Read;
use xml::reader::{XmlEvent, EventReader};

pub fn parse_time(s: &str) -> Result<NaiveTime> {
    match NaiveTime::parse_from_str(s, "%H:%M:%S") {
        Ok(t) => Ok(t),
        Err(_) => {
            Ok(NaiveTime::parse_from_str(s, "%H:%M")?)
        }
    }
}


#[macro_export]
macro_rules! impl_from_for_error {
    ($error:ident, $($orig:ident => $var:ident),*) => {
        $(
            impl From<$orig> for $error {
                fn from(err: $orig) -> $error {
                    $error::$var(err)
                }
            }
        )*
    }
}
#[macro_export]
macro_rules! xml_attrs {
    ($se:ident, $value:ident, $(parse $($name:ident),* on $struct:ident,)* $(with $($with_name:ident),* on $with_struct:ident $with_block:expr,)* $(pat $pat:pat => $block:expr),*) => {
        for ::xml::attribute::OwnedAttribute { name, $value } in $se.attributes {
            match &name.local_name as &str {
                $(
                    $(
                        stringify!($with_name) => {
                            $with_struct.$with_name = $with_block;
                        },
                    )*
                )*
                $(
                    $(
                        stringify!($name) => {
                            $struct.$name = $value.parse()?;
                        },
                    )*
                )*
                $(
                    $pat => $block,
                )*
                x => Err(
                    $crate::errors::DarwinError::Expected(
                        concat!(
                            "one of ( ",
                            $(stringify!($pat), " ",)*
                            $($(stringify!($with_name), " ",)*)*
                            $($(stringify!($name), " ",)*)*
                            ")"
                        ), 
                        x.into()
                        ))?
            }
        }
    }
}
#[macro_export]
macro_rules! xml_iter {
    ($se:ident, $reader:ident, $(pragma $pragma:ident,)* $(parse $elem_name:expr, $ty:ident as $name:ident $create_block:expr,)* $(pat $pat:pat => $block:expr),*) => {
        loop {
            match $reader.next()? {
                x @ ::xml::reader::XmlEvent::StartElement { .. } => {
                    let xse = $crate::deser::XmlStartElement::from_evt(x);
                    $(
                        if xse.name.local_name == $elem_name {
                            let $name = $ty::from_xml_iter(xse, $reader)?;
                            {
                                $create_block
                            }
                            continue;
                        }
                    )*
                    $(
                        if stringify!($pragma) == "lenient" {
                            continue;
                        }
                    )*
                    Err($crate::errors::DarwinError::UnexpectedStart(xse.name))?
                },
                $(
                    $pat => $block,
                )*
                ::xml::reader::XmlEvent::EndElement { ref name } if name == &$se.name => {
                    break;
                },
                ::xml::reader::XmlEvent::Whitespace(_) => {},
                _ if 1+1 == 1 $(|| stringify!($pragma) == "lenient")* => {},
                x => Err($crate::errors::DarwinError::UnexpectedEvent(x))?
            }
        }
    }
}
pub struct ValueElement<T>(pub T);
impl<T: FromStr> XmlDeserialize for ValueElement<T> where T::Err: Into<DarwinError> {
    fn from_xml_iter<R: Read>(se: XmlStartElement, reader: &mut EventReader<R>) -> Result<Self> {
        let mut ret: Option<T> = None;
        xml_iter! { se, reader,
            pat XmlEvent::Characters(data) => {
                ret = Some(data.parse().map_err(|e: T::Err| e.into())?);
            }
        }
        let ret = ret.ok_or(DarwinError::Missing("a value"))?;
        Ok(ValueElement(ret))
    }
}
