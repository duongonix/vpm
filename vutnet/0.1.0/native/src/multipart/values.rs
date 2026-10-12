use vut_native_sdk::{Bool, Int, NamedValue, Nominal, Runtime, Value};

use super::Form;
use crate::identity::{MultipartFieldType, MultipartFileType, MultipartType};

pub(crate) fn build(
    runtime: &Runtime,
    form: &Form,
) -> Result<Value<Nominal<MultipartType>>, vut_native_sdk::AbiError> {
    let error = runtime.copy_primitive::<Int>(form.issue.map_or(0, |issue| issue as i64))?;
    let mut fields = Vec::with_capacity(form.fields.len());
    for (name, text) in &form.fields {
        let name = runtime.copy_str(name)?;
        let text = runtime.copy_str(text)?;
        fields.push(runtime.build_data::<MultipartFieldType>(&[
            NamedValue::new("_name", &name),
            NamedValue::new("_value", &text),
        ])?);
    }
    let fields = runtime.build_list(&fields)?;
    let mut files = Vec::with_capacity(form.files.len());
    for upload in &form.files {
        let name = runtime.copy_str(&upload.name)?;
        let filename = runtime.copy_str(upload.filename.as_deref().unwrap_or_default())?;
        let has_filename = runtime.copy_primitive::<Bool>(upload.filename.is_some())?;
        let content_type = runtime.copy_str(upload.content_type.as_deref().unwrap_or_default())?;
        let has_content_type = runtime.copy_primitive::<Bool>(upload.content_type.is_some())?;
        let body = runtime.copy_bytes(&upload.bytes)?;
        files.push(runtime.build_data::<MultipartFileType>(&[
            NamedValue::new("_name", &name),
            NamedValue::new("_filename", &filename),
            NamedValue::new("_has_filename", &has_filename),
            NamedValue::new("_content_type", &content_type),
            NamedValue::new("_has_content_type", &has_content_type),
            NamedValue::new("_body", &body),
        ])?);
    }
    let files = runtime.build_list(&files)?;
    runtime.build_data::<MultipartType>(&[
        NamedValue::new("_error_code", &error),
        NamedValue::new("_fields", &fields),
        NamedValue::new("_files", &files),
    ])
}
