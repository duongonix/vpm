//! CSS declaration syntax validation for typed View `css` maps.

use cssparser::{CowRcStr, DeclarationParser, ParseError, Parser, ParserState, Token};

pub(super) fn valid_declaration(property: &str, value: &str) -> bool {
    if property.is_empty()
        || property.trim() != property
        || (property.starts_with("--") && property.len() == 2)
    {
        return false;
    }

    let declaration = format!("{property}: {value}");
    let mut input = Parser::new(&declaration);
    cssparser::parse_one_declaration(&mut input, &mut DeclarationValidator).is_ok()
}

struct DeclarationValidator;

impl<'i> DeclarationParser<'i> for DeclarationValidator {
    type Declaration = ();
    type Error = ();

    fn parse_value(
        &mut self,
        property: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<Self::Error>> {
        let custom_property = property.starts_with("--") && property.len() > 2;
        let mut has_value = false;

        while !input.is_exhausted() {
            match input.next_including_whitespace_and_comments()? {
                Token::Semicolon => return Err(ParseError::unexpected_token()),
                Token::WhiteSpace(_) | Token::Comment(_) => {}
                _ => has_value = true,
            }
        }

        if has_value || custom_property {
            Ok(())
        } else {
            Err(ParseError::unexpected_token())
        }
    }
}
