use {
    quote::ToTokens,
    syn::{Attribute, Expr, LitStr, Token, Type},
};

pub(crate) fn enum_tag_encoding_string(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    parse_enum_tag_encoding(attrs).map(|tag_encoding| tag_encoding.map(|lit| lit.value()))
}

pub(crate) fn enum_tag_encoding_type(attrs: &[Attribute]) -> syn::Result<Option<Type>> {
    parse_enum_tag_encoding(attrs)?
        .map(|lit| lit.parse::<Type>())
        .transpose()
}

pub(crate) fn variant_tag_string(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    parse_variant_tag(attrs).map(|tag| tag.map(|expr| expr.to_token_stream().to_string()))
}

fn parse_enum_tag_encoding(attrs: &[Attribute]) -> syn::Result<Option<LitStr>> {
    let mut tag_encoding = None;
    for attr in attrs {
        if !attr.path().is_ident("wincode") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag_encoding") {
                if tag_encoding.is_some() {
                    return Err(meta.error("duplicate `tag_encoding`"));
                }
                tag_encoding = Some(meta.value()?.parse()?);
            } else {
                consume_meta_input(&meta)?;
            }
            Ok(())
        })?;
    }

    Ok(tag_encoding)
}

fn parse_variant_tag(attrs: &[Attribute]) -> syn::Result<Option<Expr>> {
    let mut tag = None;
    for attr in attrs {
        if !attr.path().is_ident("wincode") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                if tag.is_some() {
                    return Err(meta.error("duplicate `tag`"));
                }
                tag = Some(meta.value()?.parse()?);
            } else {
                consume_meta_input(&meta)?;
            }
            Ok(())
        })?;
    }

    Ok(tag)
}

fn consume_meta_input(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        let value = meta.value()?;
        let _ = value.parse::<Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|nested| {
            consume_meta_input(&nested)?;
            Ok(())
        })?;
    }

    Ok(())
}
