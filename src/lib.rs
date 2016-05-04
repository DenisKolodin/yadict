extern crate curl;
extern crate rustc_serialize;

use std::env;
use std::str::Utf8Error;
use curl::ErrCode;
use curl::http::Handle;
use rustc_serialize::json::{Json, ParserError};

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
    CurlError(ErrCode),
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
            code => RequestError::UnknownError(code),
        }
    }
}

impl From<ErrCode> for RequestError {
    fn from(e: ErrCode) -> Self {
        RequestError::CurlError(e)
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

impl Api {

    fn fetch_json(&self, url: &str) -> Result<Json, RequestError> {
        let mut handle = Handle::new().ssl_verifypeer(false);
        let response = try!(handle.get(url).exec());
        let s = try!(std::str::from_utf8(response.get_body()));
        let json = try!(Json::from_str(s));
        if response.get_code() != 200 {
            let object = try!(json.as_object().ok_or(RequestError::InvalidDataFormat));
            let code_object = try!(object.get("code").ok_or(RequestError::InvalidDataFormat));
            let code = try!(code_object.as_u64().ok_or(RequestError::InvalidDataFormat));
            Err(RequestError::from(code))
        } else {
            Ok(json)
        }
    }

    pub fn get_langs(&self) -> Result<Vec<String>, RequestError> {
        let url = format!("{}/getLangs?key={}", API_URL, &self.token);
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
        let url = format!("{}/lookup?key={}&lang={}&text={}", API_URL, &self.token, lang, text);
        let json = try!(self.fetch_json(&url));
        let object = try!(json.as_object().ok_or(RequestError::InvalidDataFormat));
        Ok(Json::Object(object.to_owned()))
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
}

