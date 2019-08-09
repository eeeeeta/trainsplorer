use chrono::NaiveTime;
use crate::errors::{DarwinError, Result};
use std::str::FromStr;
use crate::deser::*;
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
pub fn build_fail(e: String) -> DarwinError {
    DarwinError::BuildFail(e)
}

#[macro_export]
macro_rules! xml_build {
    ($item:ident) => {
        let $item = $item.build().map_err($crate::util::build_fail)?;
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
macro_rules! schedule_loc_deserialize {
    (
        $type:ident, $builder:ident, $ret:ident, $value:ident,
        $(sla $sla:ident,)*
        $(cpa $cpa:ident,)* 
        $(time $($time:ident),* on $time_struct:ident,)*
        $(time_opt $($time_opt:ident),* on $time_opt_struct:ident,)*
        $(parse $($name:ident $(from $as:ident)*),* on $struct:ident,)*
        $(with $($with_name:ident $(from $with_as:ident)*),* on $with_struct:ident $with_block:expr,)*
        $(pat $pat:pat => $block:expr),*
    ) => {
        impl $crate::deser::XmlDeserialize for $type {
            fn from_xml_iter<R: ::std::io::Read>(se: $crate::deser::XmlStartElement, reader: &mut ::xml::reader::EventReader<R>) -> $crate::errors::Result<Self> {
                let mut $ret = $builder::default();
                $(
                    let mut $sla = $crate::schedule::SchedLocAttributesBuilder::default();
                )*
                $(
                    let mut $cpa = $crate::schedule::CallPtAttributesBuilder::default();
                )*
                xml_attrs! { se, $value,
                    $(
                        parse tpl, act, can on $sla,
                    )*
                    $(
                        parse $($name $(from $as)*),* on $struct,
                    )*
                    $(
                        with pta, ptd on $cpa {
                            Some($crate::util::parse_time(&$value)?)
                        },
                    )*
                    $(
                        $(
                            with $time on $time_struct {
                                $crate::util::parse_time(&$value)?
                            },
                        )*
                    )*
                    $(
                        $(
                            with $time_opt on $time_opt_struct {
                                Some($crate::util::parse_time(&$value)?)
                            },
                        )*
                    )*
                    $(
                        with $($with_name $(from $with_as)*),* on $with_struct $with_block,
                    )*
                    $(
                        pat $pat => $block,
                    )*
                    $(
                        pat "planAct" => {
                            $sla.plan_act(Some($value));
                        }
                    )*
                }
                $(
                    xml_build!($sla);
                    $ret.$sla($sla);
                )*
                $(
                    xml_build!($cpa);
                    $ret.$cpa($cpa);
                )*
                xml_iter! { se, reader, }
                xml_build!($ret);
                Ok($ret)
            }
        }
    }
}
#[macro_export]
macro_rules! xml_attrs {
    (
        $se:ident, $value:ident,
        $(parse $($name:ident $(from $as:ident)*),* on $struct:ident,)*
        $(with $($with_name:ident $(from $with_as:ident)*),* on $with_struct:ident $with_block:expr,)*
        $(pat $pat:pat => $block:expr),*
    ) => {
        for ::xml::attribute::OwnedAttribute { name, $value } in $se.attributes {
            match &name.local_name as &str {
                $(
                    $(
                        $(
                            stringify!($with_as) => {
                                $with_struct.$with_name($with_block);
                            },
                        )*
                        concat!(stringify!($with_name) $(,"INVALID",stringify!($with_as))*) => {
                            $with_struct.$with_name($with_block);
                        },
                    )*
                )*
                $(
                    $(
                        $(
                            stringify!($as) => {
                                $struct.$name($value.parse()?);
                            },
                        )*
                        concat!(stringify!($name) $(,"INVALID",stringify!($as))*) => {
                            $struct.$name($value.parse()?);
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
    (
        $se:ident,
        $reader:ident,
        $(pragma $pragma:ident,)*
        $(parse $elem_name:expr, $ty:ident as $name:ident $create_block:expr,)*
        $(pat $pat:pat => $block:expr),*
    ) => {
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
                        if stringify!($pragma) == "lenient" && !cfg!(test) {
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
                _ if 1+1 == 1 $(|| (!cfg!(test) && stringify!($pragma) == "lenient"))* => {},
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
