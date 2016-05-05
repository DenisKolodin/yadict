extern crate hyper;
extern crate rustc_serialize;

use std::env;
use std::str::Utf8Error;
use std::io::{Read, Error as IOError};
use hyper::client::Client;
use hyper::status::StatusCode;
use hyper::error::Error as HyperError;
use rustc_serialize::json::{Json, Object, ParserError};

pub const API_URL : &'static str = "https://dictionary.yandex.net/api/v1/dicservice.json";

pub struct Api {
   token: String, 
}

#[derive(Debug)]
pub enum ApiError {
    InvalidEnvironmentVar(env::VarError),
}

impl Api {
    pub fn from_token(token: &str) -> Result<Api, ApiError> {
        Ok(Api {
            token: token.to_owned(),
        })
    }

    pub fn from_env(var: &str) -> Result<Api, ApiError> {
        let token = match env::var(var) {
            Ok(tok) => tok,
            Err(e) => return Err(ApiError::InvalidEnvironmentVar(e)), 
        };
        Self::from_token(&token)
    }
}

#[derive(Debug)]
pub enum RequestError {
    KeyInvalid,
    KeyBlocked,
    DailyLimitExceeded,
    TextTooLong,
    LangNotSupported,

    InvalidDataFormat,
    UnknownError(u64),
    HyperError(HyperError),
    IOError(IOError),
    EncodingError(Utf8Error),
    ParseError(ParserError),
}

impl From<u64> for RequestError {
    // Important! It's not HTTP codes! It's codes inside JSON response.
    fn from(e: u64) -> Self {
        match e {
            401 => RequestError::KeyInvalid,
            402 => RequestError::KeyBlocked,
            403 => RequestError::DailyLimitExceeded,
            413 => RequestError::TextTooLong,
            501 => RequestError::LangNotSupported,
            xxx => RequestError::UnknownError(xxx),
        }
    }
}

impl From<HyperError> for RequestError {
    fn from(e: HyperError) -> Self {
        RequestError::HyperError(e)
    }
}

impl From<IOError> for RequestError {
    fn from(e: IOError) -> Self {
        RequestError::IOError(e)
    }
}

impl From<ParserError> for RequestError {
    fn from(e: ParserError) -> Self {
        RequestError::ParseError(e)
    }
}

impl From<Utf8Error> for RequestError {
    fn from(e: Utf8Error) -> Self {
        RequestError::EncodingError(e)
    }
}

pub struct Def {
    pub word: Word,
    pub trans: Vec<Word>,
}

pub struct Word {
    pub text: String,
    pub pos: Option<String>,
    pub ts: Option<String>,
}

fn json_to_word(object: &Object) -> Result<Word, RequestError> {
    let text = match object.get("text") {
        Some(&Json::String(ref s)) => s.to_owned(),
        _ => return Err(RequestError::InvalidDataFormat),
    };
    let pos = match object.get("pos") {
        Some(&Json::String(ref s)) => Some(s.to_owned()),
        _ => None,
    };
    let ts = match object.get("ts") {
        Some(&Json::String(ref s)) => Some(s.to_owned()),
        _ => None,
    };
    Ok(Word {
        text: text,
        pos: pos,
        ts: ts,
    })
}

impl Api {

    fn fetch_json(&self, url: &str) -> Result<Json, RequestError> {
        let url = format!("{}/{}", API_URL, url);
        let client = Client::new();
        let mut response = try!(client.get(&url).send());
        let mut s = String::new();
        try!(response.read_to_string(&mut s));
        let json = try!(Json::from_str(&s));
        if response.status != StatusCode::Ok {
            let object = try!(json.as_object().ok_or(RequestError::InvalidDataFormat));
            let code_object = try!(object.get("code").ok_or(RequestError::InvalidDataFormat));
            let code = try!(code_object.as_u64().ok_or(RequestError::InvalidDataFormat));
            Err(RequestError::from(code))
        } else {
            Ok(json)
        }
    }

    pub fn get_langs(&self) -> Result<Vec<String>, RequestError> {
        let url = format!("getLangs?key={}", &self.token);
        let json = try!(self.fetch_json(&url));
        let array = try!(json.as_array().ok_or(RequestError::InvalidDataFormat));
        let mut result = Vec::new();
        for obj in array {
            if let Some(s) = obj.as_string() {
                result.push(s.to_owned());
            }
        }
        Ok(result)
    }

    pub fn lookup(&self, lang: &str, text: &str) -> Result<Json, RequestError> {
        let url = format!("lookup?key={}&lang={}&text={}", &self.token, lang, text);
        let json = try!(self.fetch_json(&url));
        let object = try!(json.as_object().ok_or(RequestError::InvalidDataFormat));
        Ok(Json::Object(object.to_owned()))
    }

    pub fn lookup_def(&self, lang: &str, text: &str) -> Result<Vec<Def>, RequestError> {
        let json = try!(self.lookup(lang, text));
        let mut result = Vec::new();
        let object = try!(json.as_object().ok_or(RequestError::InvalidDataFormat));
        let def_obj = try!(object.get("def").ok_or(RequestError::InvalidDataFormat));
        let def_arr = try!(def_obj.as_array().ok_or(RequestError::InvalidDataFormat));
        for item in def_arr {
            let item = try!(item.as_object().ok_or(RequestError::InvalidDataFormat));
            let word = try!(json_to_word(item));
            let mut trans = Vec::new();
            let trans_obj = try!(item.get("tr").ok_or(RequestError::InvalidDataFormat));
            let trans_arr = try!(trans_obj.as_array().ok_or(RequestError::InvalidDataFormat));
            for tr in trans_arr {
                let tr_obj = try!(tr.as_object().ok_or(RequestError::InvalidDataFormat));
                let tr_word = try!(json_to_word(tr_obj));
                trans.push(tr_word);
            }
            let def = Def {
                word: word,
                trans: trans,
            };
            result.push(def);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {

    use super::Api;

    #[test]
    fn check_get_langs() {
        let api = Api::from_env("YANDEX_DICTIONARY_TOKEN").unwrap();
        let langs = api.get_langs().unwrap();
        assert!(langs.contains(&"en-ru".to_string()));
    }

    #[test]
    fn check_lookup() {
        let api = Api::from_env("YANDEX_DICTIONARY_TOKEN").unwrap();
        api.lookup("en-ru", "rust").unwrap();
    }

    #[test]
    fn check_lookup_def() {
        let api = Api::from_env("YANDEX_DICTIONARY_TOKEN").unwrap();
        for def in api.lookup_def("en-ru", "rust").unwrap() {
            assert_eq!(def.word.text, "rust");
            assert_eq!(def.word.ts.as_ref().unwrap(), "r\u{28c}st");
            assert!(def.trans.len() > 0);
            let text = &def.trans[0].text;
            let pos = def.word.pos.as_ref().unwrap();
            if pos == "noun" {
                assert_eq!(text, "\u{440}\u{436}\u{430}\u{432}\u{447}\u{438}\u{43d}\u{430}");
            } else if pos == "verb" {
                assert_eq!(text, "\u{440}\u{436}\u{430}\u{432}\u{435}\u{442}\u{44c}");
            } else if pos == "adjective" {
                assert_eq!(text, "\u{440}\u{436}\u{430}\u{432}\u{44b}\u{439}");
            } else {
                panic!("Unknown pos of 'rust' word: {}", pos);
            }
        }
    }
}

