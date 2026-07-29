use crate::ir::{CmpOp, Instruction, IrProgram};
use crate::object::{ObjectImage, RelocationKind};
use std::collections::HashMap;

const FILE_ALIGNMENT: u32 = 0x200;
const SECTION_ALIGNMENT: u32 = 0x1000;
const IMAGE_BASE: u64 = 0x140000000;
const MAX_PE_EVAL_STEPS: usize = 100_000;

pub fn emit_pe64_console(program: &IrProgram) -> Option<Vec<u8>> {
    if let Some(image) = emit_compiled_pe64_console(program) {
        return Some(image);
    }
    let plan = PePlan::from_program(program)?;
    let layout = Layout::new(
        plan.output
            .as_deref()
            .map_or(0, |message| message.len() as u32 + 1),
        plan.output.is_some(),
        false,
        false,
        false,
        false,
    );
    let text = build_text(&layout, &plan);
    let rdata = build_rdata(plan.output.as_deref().unwrap_or(""));
    let idata = build_idata(&layout);
    Some(build_image(&layout, &text, &rdata, &idata))
}

#[cfg(test)]
mod tests {
    use super::Layout;

    #[test]
    fn layout_places_write_file_count_after_compiled_rodata() {
        let layout = Layout::new(9, true, false, false, false, false);

        assert_eq!(layout.written_rva, 0x2010);
    }
}

fn emit_compiled_pe64_console(program: &IrProgram) -> Option<Vec<u8>> {
    let image = crate::object::build_win64_code_image(program)?;
    let needs_read_line = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "read_line");
    let needs_console_helpers = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "print" || relocation.symbol == "println")
        || needs_read_line
        || image.relocations.iter().any(|relocation| {
            matches!(
                relocation.symbol.as_str(),
                "write_file" | "append_file" | "file_write"
            )
        });
    let needs_string_concat = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_concat");
    let needs_virtual_alloc = needs_string_concat
        || image.relocations.iter().any(|relocation| {
            matches!(
                relocation.symbol.as_str(),
                "array_new"
                    | "array_clone"
                    | "array_reserve"
                    | "alloc"
                    | "alloc_zeroed"
                    | "alloc_array"
                    | "string_from_byte"
                    | "string_clone"
                    | "alloc_copy"
            )
        });
    let needs_file_read = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "read_file"
                | "read_file_or"
                | "read_line"
                | "write_file"
                | "file_read"
                | "file_read_to_string"
                | "file_size"
                | "file_is_empty"
        )
    });
    let needs_file_ops = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "append_file"
                | "touch_file"
                | "remove_file"
                | "file_open"
                | "file_open_write"
                | "file_open_append"
                | "file_write"
                | "file_close"
        )
    });
    let needs_file_metadata = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "file_exists" | "file_is_file" | "file_is_dir" | "file_is_empty" | "file_size"
        )
    });
    let layout = Layout::new(
        image.rodata.len() as u32,
        needs_console_helpers,
        needs_virtual_alloc || needs_file_read,
        needs_file_read,
        needs_file_ops,
        needs_file_metadata,
    );
    let text = build_compiled_text(&layout, &image)?;
    let rdata = build_compiled_rdata(&image, needs_console_helpers);
    let idata = build_idata(&layout);
    Some(build_image(&layout, &text, &rdata, &idata))
}

struct PePlan {
    output: Option<String>,
    exit_code: u32,
}

impl PePlan {
    fn from_program(program: &IrProgram) -> Option<Self> {
        let mut evaluator = PeEvaluator::new(program);
        let result = evaluator.eval_function("main", Vec::new())?;
        Some(Self {
            output: evaluator.output,
            exit_code: result.as_int()? as u32,
        })
    }
}

struct PeEvaluator<'a> {
    functions: HashMap<&'a str, &'a crate::ir::IrFunction>,
    output: Option<String>,
    steps: usize,
}

impl<'a> PeEvaluator<'a> {
    fn new(program: &'a IrProgram) -> Self {
        Self {
            functions: program
                .functions
                .iter()
                .map(|function| (function.name.as_str(), function))
                .collect(),
            output: None,
            steps: 0,
        }
    }

    fn eval_function(&mut self, name: &str, args: Vec<PeValue>) -> Option<PeValue> {
        let function = *self.functions.get(name)?;
        if function.params.len() != args.len() {
            return None;
        }

        let mut values = HashMap::new();
        let mut locals = HashMap::new();
        for (param, value) in function.params.iter().zip(args) {
            locals.insert(param.clone(), value);
        }

        let labels: HashMap<&str, usize> = function
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(idx, instruction)| match instruction {
                Instruction::Label { name } => Some((name.as_str(), idx)),
                _ => None,
            })
            .collect();

        let mut pc = 0;
        while pc < function.instructions.len() {
            if self.steps >= MAX_PE_EVAL_STEPS {
                return None;
            }
            self.steps += 1;

            match &function.instructions[pc] {
                Instruction::Const { dst, value } => {
                    values.insert(*dst, PeValue::Int(*value));
                }
                Instruction::StringConst { dst, value, .. } => {
                    values.insert(*dst, PeValue::String(value.clone()));
                }
                Instruction::And { dst, left, right } => {
                    let value = values.get(left)?.as_bool()? && values.get(right)?.as_bool()?;
                    values.insert(*dst, PeValue::Int(i64::from(value)));
                }
                Instruction::Or { dst, left, right } => {
                    let value = values.get(left)?.as_bool()? || values.get(right)?.as_bool()?;
                    values.insert(*dst, PeValue::Int(i64::from(value)));
                }
                Instruction::BitAnd { dst, left, right } => {
                    let value = values.get(left)?.as_int()? & values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::BitOr { dst, left, right } => {
                    let value = values.get(left)?.as_int()? | values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::BitXor { dst, left, right } => {
                    let value = values.get(left)?.as_int()? ^ values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::BitNot { dst, value } => {
                    let value = !values.get(value)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::ShiftLeft { dst, left, right } => {
                    let amount = u32::try_from(values.get(right)?.as_int()?).ok()?;
                    let value = values.get(left)?.as_int()?.checked_shl(amount)?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::ShiftRight { dst, left, right } => {
                    let amount = u32::try_from(values.get(right)?.as_int()?).ok()?;
                    let value = values.get(left)?.as_int()?.checked_shr(amount)?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Add { dst, left, right } => {
                    let value = values.get(left)?.as_int()? + values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Sub { dst, left, right } => {
                    let value = values.get(left)?.as_int()? - values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Mul { dst, left, right } => {
                    let value = values.get(left)?.as_int()? * values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Div { dst, left, right } => {
                    let divisor = values.get(right)?.as_int()?;
                    if divisor == 0 {
                        return None;
                    }
                    let value = values.get(left)?.as_int()? / divisor;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Rem { dst, left, right } => {
                    let divisor = values.get(right)?.as_int()?;
                    if divisor == 0 {
                        return None;
                    }
                    let value = values.get(left)?.as_int()? % divisor;
                    values.insert(*dst, PeValue::Int(value));
                }
                Instruction::Cmp {
                    dst,
                    op,
                    left,
                    right,
                } => {
                    let left = values.get(left)?.as_int()?;
                    let right = values.get(right)?.as_int()?;
                    values.insert(*dst, PeValue::Int(i64::from(eval_cmp(*op, left, right))));
                }
                Instruction::Store { local, value } => {
                    locals.insert(local.clone(), values.get(value)?.clone());
                }
                Instruction::Load { dst, local } => {
                    values.insert(*dst, locals.get(local)?.clone());
                }
                Instruction::AddressOf { dst, local } => {
                    if !locals.contains_key(local) {
                        return None;
                    }
                    values.insert(*dst, PeValue::LocalRef(local.clone()));
                }
                Instruction::Deref { dst, pointer, .. } => {
                    let PeValue::LocalRef(local) = values.get(pointer)? else {
                        return None;
                    };
                    values.insert(*dst, locals.get(local)?.clone());
                }
                Instruction::StoreDeref { pointer, value, .. } => {
                    let PeValue::LocalRef(local) = values.get(pointer)? else {
                        return None;
                    };
                    locals.insert(local.clone(), values.get(value)?.clone());
                }
                Instruction::Jump { label } => {
                    pc = *labels.get(label.as_str())?;
                    continue;
                }
                Instruction::JumpIfZero { value, label } => {
                    if values.get(value)?.as_int()? == 0 {
                        pc = *labels.get(label.as_str())?;
                        continue;
                    }
                }
                Instruction::Label { .. } => {}
                Instruction::Call {
                    dst,
                    function,
                    args,
                } if function == "print" || function == "println" => {
                    let arg = args.first()?;
                    let output = self.output.get_or_insert_with(String::new);
                    output.push_str(values.get(arg)?.as_string()?);
                    if function == "println" {
                        output.push('\n');
                    }
                    values.insert(*dst, PeValue::Int(0));
                }
                Instruction::Call {
                    dst,
                    function,
                    args,
                } if function == "string_concat" => {
                    let [left, right] = args.as_slice() else {
                        return None;
                    };
                    let value = format!(
                        "{}{}",
                        values.get(left)?.as_string()?,
                        values.get(right)?.as_string()?
                    );
                    values.insert(*dst, PeValue::String(value));
                }
                Instruction::Call {
                    dst,
                    function,
                    args,
                } => {
                    let args = args
                        .iter()
                        .map(|arg| values.get(arg).cloned())
                        .collect::<Option<Vec<_>>>()?;
                    let value = self.eval_function(function, args)?;
                    values.insert(*dst, value);
                }
                Instruction::Return { value } => {
                    return values.get(value).cloned();
                }
                _ => return None,
            }
            pc += 1;
        }

        Some(PeValue::Int(0))
    }
}

fn eval_cmp(op: CmpOp, left: i64, right: i64) -> bool {
    match op {
        CmpOp::Equal => left == right,
        CmpOp::NotEqual => left != right,
        CmpOp::Less => left < right,
        CmpOp::LessEqual => left <= right,
        CmpOp::Greater => left > right,
        CmpOp::GreaterEqual => left >= right,
    }
}

#[derive(Clone)]
enum PeValue {
    Int(i64),
    String(String),
    LocalRef(String),
}

impl PeValue {
    fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::String(_) | Self::LocalRef(_) => None,
        }
    }

    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Int(_) | Self::LocalRef(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        self.as_int().map(|value| value != 0)
    }
}

struct Layout {
    text_rva: u32,
    rdata_rva: u32,
    idata_rva: u32,
    text_raw: u32,
    message_rva: u32,
    written_rva: u32,
    import_descriptor_rva: u32,
    import_descriptor_size: u32,
    iat_size: u32,
    get_std_handle_iat: u32,
    write_file_iat: u32,
    exit_process_iat: u32,
    virtual_alloc_iat: u32,
    virtual_free_iat: u32,
    has_console_io: bool,
    has_virtual_alloc: bool,
    has_file_read: bool,
    create_file_iat: u32,
    get_file_size_iat: u32,
    read_file_iat: u32,
    close_handle_iat: u32,
    delete_file_iat: u32,
    has_file_ops: bool,
    get_file_attributes_iat: u32,
    has_file_metadata: bool,
}

impl Layout {
    fn new(
        data_len: u32,
        has_console_io: bool,
        has_virtual_alloc: bool,
        has_file_read: bool,
        has_file_ops: bool,
        has_file_metadata: bool,
    ) -> Self {
        let headers_raw = 0x200;
        let text_rva = 0x1000;
        let rdata_rva = 0x2000;
        let idata_rva = 0x3000;
        let text_raw = headers_raw;
        let message_rva = rdata_rva;
        let written_rva = align_to(message_rva + data_len, 8);
        let import_descriptor_rva = idata_rva;
        let oft_rva = import_descriptor_rva + 40;
        let file_import_count = if has_file_read {
            4
        } else if has_file_ops {
            2
        } else {
            0
        };
        let import_count = u32::from(has_console_io) * 2
            + 1
            + u32::from(has_virtual_alloc) * 2
            + file_import_count
            + u32::from(has_file_ops)
            + u32::from(has_file_metadata);
        let iat_size = (import_count + 1) * 8;
        let ft_rva = oft_rva + iat_size;
        let mut next_iat = ft_rva;
        let get_std_handle_iat = if has_console_io {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let write_file_iat = if has_console_io {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let exit_process_iat = {
            let value = next_iat;
            next_iat += 8;
            value
        };
        let virtual_alloc_iat = if has_virtual_alloc {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let virtual_free_iat = if has_virtual_alloc {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let create_file_iat = if has_file_read || has_file_ops {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let get_file_size_iat = if has_file_read {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let read_file_iat = if has_file_read {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let close_handle_iat = if has_file_read || has_file_ops {
            let value = next_iat;
            next_iat += 8;
            value
        } else {
            ft_rva
        };
        let delete_file_iat = if has_file_ops { next_iat } else { ft_rva };
        if has_file_ops {
            next_iat += 8;
        }
        let get_file_attributes_iat = if has_file_metadata { next_iat } else { ft_rva };
        Self {
            text_rva,
            rdata_rva,
            idata_rva,
            text_raw,
            message_rva,
            written_rva,
            import_descriptor_rva,
            import_descriptor_size: FILE_ALIGNMENT,
            iat_size,
            get_std_handle_iat,
            write_file_iat,
            exit_process_iat,
            virtual_alloc_iat,
            virtual_free_iat,
            has_console_io,
            has_virtual_alloc,
            has_file_read,
            create_file_iat,
            get_file_size_iat,
            read_file_iat,
            close_handle_iat,
            delete_file_iat,
            has_file_ops,
            get_file_attributes_iat,
            has_file_metadata,
        }
    }
}

fn build_text(layout: &Layout, plan: &PePlan) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    if let Some(output) = &plan.output {
        code.push(0xb9);
        code.extend_from_slice(&(-11_i32).to_le_bytes());
        emit_call_iat(&mut code, layout, layout.get_std_handle_iat);
        code.extend_from_slice(&[0x48, 0x89, 0xc1]);
        emit_lea(&mut code, layout, &[0x48, 0x8d, 0x15], layout.message_rva);
        code.extend_from_slice(&[0x41, 0xb8]);
        code.extend_from_slice(&(output.len() as u32).to_le_bytes());
        emit_lea(&mut code, layout, &[0x4c, 0x8d, 0x0d], layout.written_rva);
        code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]);
        emit_call_iat(&mut code, layout, layout.write_file_iat);
    }
    code.push(0xb9);
    code.extend_from_slice(&plan.exit_code.to_le_bytes());
    emit_call_iat(&mut code, layout, layout.exit_process_iat);
    pad_to(&mut code, FILE_ALIGNMENT as usize);
    code
}

fn build_compiled_text(layout: &Layout, image: &ObjectImage) -> Option<Vec<u8>> {
    let entry_len = 21_u32;
    let function_base = align_to(entry_len, 16);
    let needs_bounds_check = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "__geo_bounds_check");
    let needs_print = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "print" || relocation.symbol == "println");
    let needs_println = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "println");
    let needs_read_line = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "read_line");
    let needs_string_concat = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_concat");
    let needs_string_len = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_len")
        || needs_string_concat;
    let needs_string_utf8_len = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_len");
    let needs_string_utf8_codepoint_at = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_codepoint_at");
    let needs_string_is_utf8 = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_utf8");
    let needs_string_utf8_is_valid = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_is_valid");
    let needs_string_utf8_byte_offset = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_byte_offset");
    let needs_string_utf8_next_offset = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_next_offset");
    let needs_string_utf8_prev_offset = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_prev_offset");
    let needs_string_utf8_index_at = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_index_at");
    let needs_string_utf8_is_boundary = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_utf8_is_boundary");
    let needs_array_new = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_new");
    let needs_array_clone = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_clone");
    let needs_array_reserve = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_reserve");
    let needs_array_first = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_first");
    let needs_array_last = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_last");
    let needs_array_fill = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_fill");
    let needs_array_reverse = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_reverse");
    let needs_array_index_of = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_index_of");
    let needs_array_last_index_of = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_last_index_of");
    let needs_array_contains = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_contains");
    let needs_array_count = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_count");
    let needs_array_len = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_len");
    let needs_array_capacity = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_capacity");
    let needs_array_is_empty = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_is_empty");
    let needs_array_get = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_get");
    let needs_array_set = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_set");
    let needs_array_push = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_push");
    let needs_array_clear = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_clear");
    let needs_array_free = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "array_free");
    let needs_string_byte_at = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_byte_at");
    let needs_string_compare = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_compare");
    let needs_string_contains = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_contains");
    let needs_string_starts_with = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_starts_with");
    let needs_string_ends_with = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_ends_with");
    let needs_string_eq = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_eq");
    let needs_string_not_eq = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_not_eq");
    let needs_string_less = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_less");
    let needs_string_less_or_equal = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_less_or_equal");
    let needs_string_greater = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_greater");
    let needs_string_greater_or_equal = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_greater_or_equal");
    let needs_string_is_empty = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_empty");
    let needs_string_is_ascii = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii");
    let needs_string_is_ascii_digit = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_digit");
    let needs_string_is_ascii_hex_digit = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_hex_digit");
    let needs_string_is_ascii_alpha = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_alpha");
    let needs_string_is_ascii_lower = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_lower");
    let needs_string_is_ascii_upper = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_upper");
    let needs_string_is_ascii_alnum = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_alnum");
    let needs_string_is_ascii_identifier = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_identifier");
    let needs_string_is_ascii_whitespace = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_is_ascii_whitespace");
    let needs_string_find_byte = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_find_byte");
    let needs_string_last_find_byte = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_last_find_byte");
    let needs_string_index_of = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_index_of");
    let needs_string_last_index_of = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_last_index_of");
    let needs_string_count = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_count");
    let needs_string_parse_int = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_parse_int");
    let needs_alloc = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "array_new" | "alloc" | "alloc_zeroed" | "alloc_array"
        )
    });
    let needs_mem_copy = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_copy");
    let needs_mem_zero = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_zero");
    let needs_mem_move = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_move");
    let needs_mem_fill = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_fill");
    let needs_mem_find = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_find");
    let needs_mem_compare = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_compare");
    let needs_mem_equal = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_equal");
    let needs_mem_is_zero = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_is_zero");
    let needs_mem_reverse = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "mem_reverse");
    let needs_string_from_byte = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_from_byte");
    let needs_string_clone = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_clone");
    let needs_string_slice = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_slice");
    let needs_alloc_copy = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "alloc_copy");
    let needs_file_read = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "read_file" | "read_file_or" | "file_read" | "file_read_to_string"
        )
    });
    let needs_file_read_or = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "read_file_or");
    let needs_append_file = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "append_file");
    let needs_touch_file = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "touch_file");
    let needs_remove_file = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "remove_file");
    let needs_file_open = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "file_open" | "file_open_write" | "file_open_append"
        )
    });
    let needs_file_write = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "file_write");
    let needs_file_close = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "file_close");
    let needs_file_read_to_string = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "file_read" | "file_read_to_string" | "file_size" | "file_is_empty"
        )
    });
    let needs_file_metadata = image.relocations.iter().any(|relocation| {
        matches!(
            relocation.symbol.as_str(),
            "file_exists" | "file_is_file" | "file_is_dir" | "file_is_empty" | "file_size"
        )
    });
    let needs_process_exit = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "exit_geo");
    let newline_rva = if needs_println {
        Some(layout.rdata_rva + image.rodata.len() as u32)
    } else {
        None
    };
    let main = image
        .functions
        .iter()
        .find(|function| function.name == "main")?;
    let main_rva = layout.text_rva + function_base + main.offset as u32;

    let mut code = Vec::new();
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    emit_direct_call(&mut code, layout.text_rva, main_rva);
    code.extend_from_slice(&[0x89, 0xc1]);
    emit_call_iat(&mut code, layout, layout.exit_process_iat);
    while code.len() < function_base as usize {
        code.push(0xcc);
    }
    code.extend_from_slice(&image.text);
    let mut helpers = PeHelperRvas::default();
    if needs_bounds_check
        || needs_print
        || needs_string_concat
        || needs_string_len
        || needs_string_utf8_len
        || needs_string_utf8_codepoint_at
        || needs_string_is_utf8
        || needs_string_utf8_is_valid
        || needs_string_utf8_byte_offset
        || needs_string_utf8_next_offset
        || needs_string_utf8_prev_offset
        || needs_string_utf8_index_at
        || needs_string_utf8_is_boundary
        || needs_array_new
        || needs_array_clone
        || needs_array_reserve
        || needs_array_first
        || needs_array_last
        || needs_array_fill
        || needs_array_reverse
        || needs_array_index_of
        || needs_array_last_index_of
        || needs_array_contains
        || needs_array_count
        || needs_array_len
        || needs_array_capacity
        || needs_array_is_empty
        || needs_array_get
        || needs_array_set
        || needs_array_push
        || needs_array_clear
        || needs_array_free
        || needs_string_byte_at
        || needs_string_compare
        || needs_string_contains
        || needs_string_starts_with
        || needs_string_ends_with
        || needs_string_eq
        || needs_string_not_eq
        || needs_string_less
        || needs_string_less_or_equal
        || needs_string_greater
        || needs_string_greater_or_equal
        || needs_string_is_empty
        || needs_string_is_ascii
        || needs_string_is_ascii_digit
        || needs_string_is_ascii_hex_digit
        || needs_string_is_ascii_alpha
        || needs_string_is_ascii_lower
        || needs_string_is_ascii_upper
        || needs_string_is_ascii_alnum
        || needs_string_is_ascii_identifier
        || needs_string_is_ascii_whitespace
        || needs_string_find_byte
        || needs_string_last_find_byte
        || needs_string_index_of
        || needs_string_last_index_of
        || needs_string_count
        || needs_string_parse_int
        || needs_alloc
        || needs_mem_copy
        || needs_mem_zero
        || needs_mem_move
        || needs_mem_fill
        || needs_mem_find
        || needs_mem_compare
        || needs_mem_equal
        || needs_mem_is_zero
        || needs_mem_reverse
        || needs_string_from_byte
        || needs_string_clone
        || needs_string_slice
        || needs_alloc_copy
        || needs_file_read
        || needs_file_read_or
        || needs_append_file
        || needs_touch_file
        || needs_remove_file
        || needs_file_open
        || needs_file_write
        || needs_file_close
        || needs_file_read_to_string
        || needs_file_metadata
        || needs_read_line
        || needs_process_exit
    {
        let helper_start_rva = layout.text_rva + align_to(code.len() as u32, 16);
        while layout.text_rva + (code.len() as u32) < helper_start_rva {
            code.push(0xcc);
        }
    }
    if needs_string_len {
        helpers.string_len = Some(layout.text_rva + code.len() as u32);
        emit_string_len_helper(&mut code);
    }
    if needs_string_utf8_len {
        helpers.string_utf8_len = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_len_helper(&mut code);
    }
    if needs_string_utf8_codepoint_at {
        helpers.string_utf8_codepoint_at = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_codepoint_at_helper(&mut code);
    }
    if needs_string_is_utf8 {
        helpers.string_is_utf8 = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_valid_helper(&mut code);
    }
    if needs_string_utf8_is_valid {
        helpers.string_utf8_is_valid = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_valid_helper(&mut code);
    }
    if needs_string_utf8_byte_offset {
        helpers.string_utf8_byte_offset = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_navigation_helper(&mut code, Utf8NavigationKindPe::ByteOffset);
    }
    if needs_string_utf8_next_offset {
        helpers.string_utf8_next_offset = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_navigation_helper(&mut code, Utf8NavigationKindPe::NextOffset);
    }
    if needs_string_utf8_prev_offset {
        helpers.string_utf8_prev_offset = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_navigation_helper(&mut code, Utf8NavigationKindPe::PrevOffset);
    }
    if needs_string_utf8_index_at {
        helpers.string_utf8_index_at = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_navigation_helper(&mut code, Utf8NavigationKindPe::IndexAt);
    }
    if needs_string_utf8_is_boundary {
        helpers.string_utf8_is_boundary = Some(layout.text_rva + code.len() as u32);
        emit_string_utf8_navigation_helper(&mut code, Utf8NavigationKindPe::IsBoundary);
    }
    if needs_array_new {
        helpers.array_new = Some(layout.text_rva + code.len() as u32);
        emit_array_new_helper(&mut code, layout);
    }
    if needs_array_clone {
        helpers.array_clone = Some(layout.text_rva + code.len() as u32);
        emit_array_clone_helper(&mut code, layout);
    }
    if needs_array_reserve {
        helpers.array_reserve = Some(layout.text_rva + code.len() as u32);
        emit_array_reserve_helper(&mut code, layout);
    }
    if needs_array_first {
        helpers.array_first = Some(layout.text_rva + code.len() as u32);
        emit_array_first_helper(&mut code);
    }
    if needs_array_last {
        helpers.array_last = Some(layout.text_rva + code.len() as u32);
        emit_array_last_helper(&mut code);
    }
    if needs_array_fill {
        helpers.array_fill = Some(layout.text_rva + code.len() as u32);
        emit_array_fill_helper(&mut code);
    }
    if needs_array_reverse {
        helpers.array_reverse = Some(layout.text_rva + code.len() as u32);
        emit_array_reverse_helper(&mut code);
    }
    if needs_array_index_of {
        helpers.array_index_of = Some(layout.text_rva + code.len() as u32);
        emit_array_index_helper(&mut code, false, false);
    }
    if needs_array_last_index_of {
        helpers.array_last_index_of = Some(layout.text_rva + code.len() as u32);
        emit_array_index_helper(&mut code, true, false);
    }
    if needs_array_contains {
        helpers.array_contains = Some(layout.text_rva + code.len() as u32);
        emit_array_index_helper(&mut code, false, true);
    }
    if needs_array_count {
        helpers.array_count = Some(layout.text_rva + code.len() as u32);
        emit_array_index_helper(&mut code, false, true);
    }
    if needs_array_len {
        helpers.array_len = Some(layout.text_rva + code.len() as u32);
        emit_array_len_helper(&mut code);
    }
    if needs_array_capacity {
        helpers.array_capacity = Some(layout.text_rva + code.len() as u32);
        emit_array_capacity_helper(&mut code);
    }
    if needs_array_is_empty {
        helpers.array_is_empty = Some(layout.text_rva + code.len() as u32);
        emit_array_is_empty_helper(&mut code);
    }
    if needs_array_get {
        helpers.array_get = Some(layout.text_rva + code.len() as u32);
        emit_array_get_helper(&mut code);
    }
    if needs_array_set {
        helpers.array_set = Some(layout.text_rva + code.len() as u32);
        emit_array_set_helper(&mut code);
    }
    if needs_array_push {
        helpers.array_push = Some(layout.text_rva + code.len() as u32);
        emit_array_push_helper(&mut code);
    }
    if needs_array_clear {
        helpers.array_clear = Some(layout.text_rva + code.len() as u32);
        emit_array_clear_helper(&mut code);
    }
    if needs_array_free {
        helpers.array_free = Some(layout.text_rva + code.len() as u32);
        emit_array_free_helper(&mut code, layout);
    }
    if needs_string_concat {
        let string_len = helpers.string_len?;
        helpers.string_concat = Some(layout.text_rva + code.len() as u32);
        emit_string_concat_helper(&mut code, layout, string_len);
    }
    if needs_string_byte_at {
        helpers.string_byte_at = Some(layout.text_rva + code.len() as u32);
        emit_string_byte_at_helper(&mut code);
    }
    if needs_string_compare {
        helpers.string_compare = Some(layout.text_rva + code.len() as u32);
        emit_string_compare_helper(&mut code);
    }
    if needs_string_contains {
        helpers.string_contains = Some(layout.text_rva + code.len() as u32);
        emit_string_contains_helper(&mut code);
    }
    if needs_string_starts_with {
        helpers.string_starts_with = Some(layout.text_rva + code.len() as u32);
        emit_string_starts_with_helper(&mut code);
    }
    if needs_string_ends_with {
        helpers.string_ends_with = Some(layout.text_rva + code.len() as u32);
        emit_string_ends_with_helper(&mut code);
    }
    if needs_string_eq {
        helpers.string_eq = Some(layout.text_rva + code.len() as u32);
        emit_string_eq_helper(&mut code);
    }
    if needs_string_not_eq {
        helpers.string_not_eq = Some(layout.text_rva + code.len() as u32);
        emit_string_not_eq_helper(&mut code);
    }
    if needs_string_less {
        helpers.string_less = Some(layout.text_rva + code.len() as u32);
        emit_string_less_helper(&mut code);
    }
    if needs_string_less_or_equal {
        helpers.string_less_or_equal = Some(layout.text_rva + code.len() as u32);
        emit_string_less_or_equal_helper(&mut code);
    }
    if needs_string_greater {
        helpers.string_greater = Some(layout.text_rva + code.len() as u32);
        emit_string_greater_helper(&mut code);
    }
    if needs_string_greater_or_equal {
        helpers.string_greater_or_equal = Some(layout.text_rva + code.len() as u32);
        emit_string_greater_or_equal_helper(&mut code);
    }
    if needs_string_is_empty {
        helpers.string_is_empty = Some(layout.text_rva + code.len() as u32);
        emit_string_is_empty_helper(&mut code);
    }
    if needs_string_is_ascii {
        helpers.string_is_ascii = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_helper(&mut code);
    }
    if needs_string_is_ascii_digit {
        helpers.string_is_ascii_digit = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_digit_helper(&mut code);
    }
    if needs_string_is_ascii_hex_digit {
        helpers.string_is_ascii_hex_digit = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_hex_digit_helper(&mut code);
    }
    if needs_string_is_ascii_alpha {
        helpers.string_is_ascii_alpha = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_alpha_helper(&mut code);
    }
    if needs_string_is_ascii_lower {
        helpers.string_is_ascii_lower = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_lower_helper(&mut code);
    }
    if needs_string_is_ascii_upper {
        helpers.string_is_ascii_upper = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_upper_helper(&mut code);
    }
    if needs_string_is_ascii_alnum {
        helpers.string_is_ascii_alnum = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_alnum_helper(&mut code);
    }
    if needs_string_is_ascii_identifier {
        helpers.string_is_ascii_identifier = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_identifier_helper(&mut code);
    }
    if needs_string_is_ascii_whitespace {
        helpers.string_is_ascii_whitespace = Some(layout.text_rva + code.len() as u32);
        emit_string_is_ascii_whitespace_helper(&mut code);
    }
    if needs_string_find_byte {
        helpers.string_find_byte = Some(layout.text_rva + code.len() as u32);
        emit_string_find_byte_helper(&mut code);
    }
    if needs_string_last_find_byte {
        helpers.string_last_find_byte = Some(layout.text_rva + code.len() as u32);
        emit_string_last_find_byte_helper(&mut code);
    }
    if needs_string_index_of {
        helpers.string_index_of = Some(layout.text_rva + code.len() as u32);
        emit_string_index_of_helper(&mut code);
    }
    if needs_string_last_index_of {
        helpers.string_last_index_of = Some(layout.text_rva + code.len() as u32);
        emit_string_last_index_of_helper(&mut code);
    }
    if needs_string_count {
        helpers.string_count = Some(layout.text_rva + code.len() as u32);
        emit_string_count_helper(&mut code);
    }
    if needs_string_parse_int {
        helpers.string_parse_int = Some(layout.text_rva + code.len() as u32);
        emit_string_parse_int_helper(&mut code);
    }
    if needs_bounds_check {
        helpers.bounds_check = Some(layout.text_rva + code.len() as u32);
        emit_bounds_check_helper(&mut code, layout);
    }
    if needs_print {
        helpers.print = Some(layout.text_rva + code.len() as u32);
        emit_print_helper(&mut code, layout);
    }
    if needs_println {
        let print_rva = helpers.print?;
        helpers.println = Some(layout.text_rva + code.len() as u32);
        emit_println_helper(&mut code, layout, print_rva, newline_rva?);
    }
    if needs_alloc {
        helpers.alloc = Some(layout.text_rva + code.len() as u32);
        let array = image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "alloc_array");
        emit_alloc_helper(&mut code, layout, array);
    }
    if needs_mem_copy {
        helpers.mem_copy = Some(layout.text_rva + code.len() as u32);
        emit_mem_copy_helper(&mut code);
    }
    if needs_mem_zero {
        helpers.mem_zero = Some(layout.text_rva + code.len() as u32);
        emit_mem_zero_helper(&mut code);
    }
    if needs_mem_move {
        helpers.mem_move = Some(layout.text_rva + code.len() as u32);
        emit_mem_move_helper(&mut code);
    }
    if needs_mem_fill {
        helpers.mem_fill = Some(layout.text_rva + code.len() as u32);
        emit_mem_fill_helper(&mut code);
    }
    if needs_mem_find {
        helpers.mem_find = Some(layout.text_rva + code.len() as u32);
        emit_mem_find_helper(&mut code);
    }
    if needs_mem_compare {
        helpers.mem_compare = Some(layout.text_rva + code.len() as u32);
        emit_mem_compare_helper(&mut code);
    }
    if needs_mem_equal {
        helpers.mem_equal = Some(layout.text_rva + code.len() as u32);
        emit_mem_equal_helper(&mut code);
    }
    if needs_mem_is_zero {
        helpers.mem_is_zero = Some(layout.text_rva + code.len() as u32);
        emit_mem_is_zero_helper(&mut code);
    }
    if needs_mem_reverse {
        helpers.mem_reverse = Some(layout.text_rva + code.len() as u32);
        emit_mem_reverse_helper(&mut code);
    }
    if needs_string_from_byte {
        helpers.string_from_byte = Some(layout.text_rva + code.len() as u32);
        emit_string_from_byte_helper(&mut code, layout);
    }
    if needs_string_clone {
        helpers.string_clone = Some(layout.text_rva + code.len() as u32);
        emit_string_clone_helper(&mut code, layout);
    }
    if image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_slice")
    {
        helpers.string_slice = Some(layout.text_rva + code.len() as u32);
        emit_string_slice_helper(&mut code, layout);
    }
    if needs_alloc_copy {
        helpers.alloc_copy = Some(layout.text_rva + code.len() as u32);
        emit_alloc_copy_helper(&mut code, layout);
    }
    if needs_process_exit {
        helpers.exit_process = Some(layout.text_rva + code.len() as u32);
        emit_process_exit_helper(&mut code, layout);
    }
    if needs_file_read {
        helpers.read_file = Some(layout.text_rva + code.len() as u32);
        emit_read_file_helper(&mut code, layout);
    }
    if needs_file_read_or {
        let read_file = helpers.read_file?;
        helpers.read_file_or = Some(layout.text_rva + code.len() as u32);
        emit_read_file_or_helper(&mut code, layout, read_file);
    }
    if image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "write_file")
    {
        helpers.write_file = Some(layout.text_rva + code.len() as u32);
        emit_write_file_helper(&mut code, layout);
    }
    if needs_append_file {
        helpers.append_file = Some(layout.text_rva + code.len() as u32);
        emit_append_file_helper(&mut code, layout);
    }
    if needs_touch_file {
        helpers.touch_file = Some(layout.text_rva + code.len() as u32);
        emit_touch_file_helper(&mut code, layout);
    }
    if needs_remove_file {
        helpers.remove_file = Some(layout.text_rva + code.len() as u32);
        emit_remove_file_helper(&mut code, layout);
    }
    if needs_file_open {
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_open")
        {
            helpers.file_open = Some(layout.text_rva + code.len() as u32);
            emit_file_open_helper(&mut code, layout, 0x8000_0000, 1, 3);
        }
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_open_write")
        {
            helpers.file_open_write = Some(layout.text_rva + code.len() as u32);
            emit_file_open_helper(&mut code, layout, 0x4000_0000, 0, 2);
        }
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_open_append")
        {
            helpers.file_open_append = Some(layout.text_rva + code.len() as u32);
            emit_file_open_helper(&mut code, layout, 4, 0, 4);
        }
    }
    if needs_file_write {
        helpers.file_write = Some(layout.text_rva + code.len() as u32);
        emit_file_write_helper(&mut code, layout);
    }
    if needs_file_close {
        helpers.file_close = Some(layout.text_rva + code.len() as u32);
        emit_file_close_helper(&mut code, layout);
    }
    if needs_file_read_to_string {
        helpers.file_read_to_string = Some(layout.text_rva + code.len() as u32);
        emit_file_read_to_string_helper(&mut code, layout);
    }
    if needs_file_metadata {
        if image
            .relocations
            .iter()
            .any(|relocation| matches!(relocation.symbol.as_str(), "file_size" | "file_is_empty"))
        {
            helpers.file_size = Some(layout.text_rva + code.len() as u32);
            emit_file_size_helper(&mut code, layout);
        }
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_is_empty")
        {
            let file_size = helpers.file_size?;
            helpers.file_is_empty = Some(layout.text_rva + code.len() as u32);
            emit_file_is_empty_helper(&mut code, layout, file_size);
        }
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_is_file")
        {
            helpers.file_is_file = Some(layout.text_rva + code.len() as u32);
            emit_file_attribute_helper(&mut code, layout, false);
        }
        if image
            .relocations
            .iter()
            .any(|relocation| relocation.symbol == "file_is_dir")
        {
            helpers.file_is_dir = Some(layout.text_rva + code.len() as u32);
            emit_file_attribute_helper(&mut code, layout, true);
        }
    }
    if needs_file_metadata {
        helpers.file_exists = Some(layout.text_rva + code.len() as u32);
        emit_file_exists_helper(&mut code, layout);
    }
    if needs_read_line {
        helpers.read_line = Some(layout.text_rva + code.len() as u32);
        emit_read_line_helper(&mut code, layout);
    }
    patch_compiled_relocations(&mut code, layout, image, function_base, &helpers)?;
    pad_to(&mut code, FILE_ALIGNMENT as usize);
    Some(code)
}

fn patch_compiled_relocations(
    code: &mut [u8],
    layout: &Layout,
    image: &ObjectImage,
    function_base: u32,
    helpers: &PeHelperRvas,
) -> Option<()> {
    for relocation in &image.relocations {
        let relocation_offset = function_base + relocation.offset as u32;
        let target_rva =
            compiled_symbol_rva(layout, image, function_base, helpers, &relocation.symbol)?;
        let next_rva = layout.text_rva + relocation_offset + 4;
        let addend = match relocation.kind {
            RelocationKind::Pc32 | RelocationKind::Plt32 => rel32(target_rva, next_rva),
        };
        let offset = relocation_offset as usize;
        code[offset..offset + 4].copy_from_slice(&addend.to_le_bytes());
    }
    Some(())
}

fn compiled_symbol_rva(
    layout: &Layout,
    image: &ObjectImage,
    function_base: u32,
    helpers: &PeHelperRvas,
    symbol: &str,
) -> Option<u32> {
    if symbol == "__geo_bounds_check" {
        return helpers.bounds_check;
    }
    if symbol == "print" {
        return helpers.print;
    }
    if symbol == "println" {
        return helpers.println;
    }
    if symbol == "string_concat" {
        return helpers.string_concat;
    }
    if symbol == "string_len" {
        return helpers.string_len;
    }
    if symbol == "string_utf8_len" {
        return helpers.string_utf8_len;
    }
    if symbol == "string_utf8_codepoint_at" {
        return helpers.string_utf8_codepoint_at;
    }
    if symbol == "string_is_utf8" {
        return helpers.string_is_utf8;
    }
    if symbol == "string_utf8_is_valid" {
        return helpers.string_utf8_is_valid;
    }
    if symbol == "string_utf8_byte_offset" {
        return helpers.string_utf8_byte_offset;
    }
    if symbol == "string_utf8_next_offset" {
        return helpers.string_utf8_next_offset;
    }
    if symbol == "string_utf8_prev_offset" {
        return helpers.string_utf8_prev_offset;
    }
    if symbol == "string_utf8_index_at" {
        return helpers.string_utf8_index_at;
    }
    if symbol == "string_utf8_is_boundary" {
        return helpers.string_utf8_is_boundary;
    }
    if symbol == "array_new" {
        return helpers.array_new;
    }
    if symbol == "array_clone" {
        return helpers.array_clone;
    }
    if symbol == "array_reserve" {
        return helpers.array_reserve;
    }
    if symbol == "array_first" {
        return helpers.array_first;
    }
    if symbol == "array_last" {
        return helpers.array_last;
    }
    if symbol == "array_fill" {
        return helpers.array_fill;
    }
    if symbol == "array_reverse" {
        return helpers.array_reverse;
    }
    if symbol == "array_index_of" {
        return helpers.array_index_of;
    }
    if symbol == "array_last_index_of" {
        return helpers.array_last_index_of;
    }
    if symbol == "array_contains" {
        return helpers.array_contains;
    }
    if symbol == "array_count" {
        return helpers.array_count;
    }
    if symbol == "array_len" {
        return helpers.array_len;
    }
    if symbol == "array_capacity" {
        return helpers.array_capacity;
    }
    if symbol == "array_is_empty" {
        return helpers.array_is_empty;
    }
    if symbol == "array_get" {
        return helpers.array_get;
    }
    if symbol == "array_set" {
        return helpers.array_set;
    }
    if symbol == "array_push" {
        return helpers.array_push;
    }
    if symbol == "array_clear" {
        return helpers.array_clear;
    }
    if symbol == "array_free" {
        return helpers.array_free;
    }
    if symbol == "string_byte_at" {
        return helpers.string_byte_at;
    }
    if symbol == "string_compare" {
        return helpers.string_compare;
    }
    if symbol == "string_contains" {
        return helpers.string_contains;
    }
    if symbol == "string_starts_with" {
        return helpers.string_starts_with;
    }
    if symbol == "string_ends_with" {
        return helpers.string_ends_with;
    }
    if symbol == "string_eq" {
        return helpers.string_eq;
    }
    if symbol == "string_not_eq" {
        return helpers.string_not_eq;
    }
    if symbol == "string_less" {
        return helpers.string_less;
    }
    if symbol == "string_less_or_equal" {
        return helpers.string_less_or_equal;
    }
    if symbol == "string_greater" {
        return helpers.string_greater;
    }
    if symbol == "string_greater_or_equal" {
        return helpers.string_greater_or_equal;
    }
    if symbol == "string_is_empty" {
        return helpers.string_is_empty;
    }
    if symbol == "string_is_ascii" {
        return helpers.string_is_ascii;
    }
    if symbol == "string_is_ascii_digit" {
        return helpers.string_is_ascii_digit;
    }
    if symbol == "string_is_ascii_hex_digit" {
        return helpers.string_is_ascii_hex_digit;
    }
    if symbol == "string_is_ascii_alpha" {
        return helpers.string_is_ascii_alpha;
    }
    if symbol == "string_is_ascii_lower" {
        return helpers.string_is_ascii_lower;
    }
    if symbol == "string_is_ascii_upper" {
        return helpers.string_is_ascii_upper;
    }
    if symbol == "string_is_ascii_alnum" {
        return helpers.string_is_ascii_alnum;
    }
    if symbol == "string_is_ascii_identifier" {
        return helpers.string_is_ascii_identifier;
    }
    if symbol == "string_is_ascii_whitespace" {
        return helpers.string_is_ascii_whitespace;
    }
    if symbol == "string_find_byte" {
        return helpers.string_find_byte;
    }
    if symbol == "string_last_find_byte" {
        return helpers.string_last_find_byte;
    }
    if symbol == "string_index_of" {
        return helpers.string_index_of;
    }
    if symbol == "string_last_index_of" {
        return helpers.string_last_index_of;
    }
    if symbol == "string_count" {
        return helpers.string_count;
    }
    if symbol == "string_parse_int" {
        return helpers.string_parse_int;
    }
    if matches!(symbol, "alloc" | "alloc_zeroed" | "alloc_array") {
        return helpers.alloc;
    }
    if symbol == "exit_geo" {
        return helpers.exit_process;
    }
    if symbol == "read_file" {
        return helpers.read_file;
    }
    if symbol == "read_file_or" {
        return helpers.read_file_or;
    }
    if symbol == "write_file" {
        return helpers.write_file;
    }
    if symbol == "append_file" {
        return helpers.append_file;
    }
    if symbol == "touch_file" {
        return helpers.touch_file;
    }
    if symbol == "remove_file" {
        return helpers.remove_file;
    }
    if symbol == "file_open" {
        return helpers.file_open;
    }
    if symbol == "file_open_write" {
        return helpers.file_open_write;
    }
    if symbol == "file_open_append" {
        return helpers.file_open_append;
    }
    if symbol == "file_write" {
        return helpers.file_write;
    }
    if symbol == "file_close" {
        return helpers.file_close;
    }
    if symbol == "file_read_to_string" {
        return helpers.file_read_to_string;
    }
    if symbol == "file_read" {
        return helpers.file_read_to_string;
    }
    if symbol == "file_exists" {
        return helpers.file_exists;
    }
    if symbol == "file_is_file" {
        return helpers.file_is_file;
    }
    if symbol == "file_is_dir" {
        return helpers.file_is_dir;
    }
    if symbol == "file_is_empty" {
        return helpers.file_is_empty;
    }
    if symbol == "file_size" {
        return helpers.file_size;
    }
    if symbol == "read_line" {
        return helpers.read_line;
    }
    if symbol == "mem_copy" {
        return helpers.mem_copy;
    }
    if symbol == "mem_zero" {
        return helpers.mem_zero;
    }
    if symbol == "mem_move" {
        return helpers.mem_move;
    }
    if symbol == "mem_fill" {
        return helpers.mem_fill;
    }
    if symbol == "mem_find" {
        return helpers.mem_find;
    }
    if symbol == "mem_compare" {
        return helpers.mem_compare;
    }
    if symbol == "mem_equal" {
        return helpers.mem_equal;
    }
    if symbol == "mem_is_zero" {
        return helpers.mem_is_zero;
    }
    if symbol == "mem_reverse" {
        return helpers.mem_reverse;
    }
    if symbol == "string_from_byte" {
        return helpers.string_from_byte;
    }
    if symbol == "string_clone" {
        return helpers.string_clone;
    }
    if symbol == "string_slice" {
        return helpers.string_slice;
    }
    if symbol == "alloc_copy" {
        return helpers.alloc_copy;
    }
    if let Some(function) = image
        .functions
        .iter()
        .find(|function| function.name == symbol)
    {
        return Some(layout.text_rva + function_base + function.offset as u32);
    }
    if let Some(data) = image.data_symbols.iter().find(|data| data.name == symbol) {
        return Some(layout.rdata_rva + data.offset as u32);
    }
    None
}

#[derive(Default)]
struct PeHelperRvas {
    bounds_check: Option<u32>,
    print: Option<u32>,
    println: Option<u32>,
    exit_process: Option<u32>,
    alloc: Option<u32>,
    read_file: Option<u32>,
    read_file_or: Option<u32>,
    write_file: Option<u32>,
    append_file: Option<u32>,
    touch_file: Option<u32>,
    remove_file: Option<u32>,
    file_open: Option<u32>,
    file_open_write: Option<u32>,
    file_open_append: Option<u32>,
    file_write: Option<u32>,
    file_close: Option<u32>,
    file_read_to_string: Option<u32>,
    file_exists: Option<u32>,
    file_is_file: Option<u32>,
    file_is_dir: Option<u32>,
    file_is_empty: Option<u32>,
    file_size: Option<u32>,
    read_line: Option<u32>,
    mem_copy: Option<u32>,
    mem_zero: Option<u32>,
    mem_move: Option<u32>,
    mem_fill: Option<u32>,
    mem_find: Option<u32>,
    mem_compare: Option<u32>,
    mem_equal: Option<u32>,
    mem_is_zero: Option<u32>,
    mem_reverse: Option<u32>,
    string_from_byte: Option<u32>,
    string_clone: Option<u32>,
    string_slice: Option<u32>,
    alloc_copy: Option<u32>,
    string_concat: Option<u32>,
    string_len: Option<u32>,
    string_utf8_len: Option<u32>,
    string_utf8_codepoint_at: Option<u32>,
    string_is_utf8: Option<u32>,
    string_utf8_is_valid: Option<u32>,
    string_utf8_byte_offset: Option<u32>,
    string_utf8_next_offset: Option<u32>,
    string_utf8_prev_offset: Option<u32>,
    string_utf8_index_at: Option<u32>,
    string_utf8_is_boundary: Option<u32>,
    array_new: Option<u32>,
    array_clone: Option<u32>,
    array_reserve: Option<u32>,
    array_first: Option<u32>,
    array_last: Option<u32>,
    array_fill: Option<u32>,
    array_reverse: Option<u32>,
    array_index_of: Option<u32>,
    array_last_index_of: Option<u32>,
    array_contains: Option<u32>,
    array_count: Option<u32>,
    array_len: Option<u32>,
    array_capacity: Option<u32>,
    array_is_empty: Option<u32>,
    array_get: Option<u32>,
    array_set: Option<u32>,
    array_push: Option<u32>,
    array_clear: Option<u32>,
    array_free: Option<u32>,
    string_byte_at: Option<u32>,
    string_compare: Option<u32>,
    string_contains: Option<u32>,
    string_starts_with: Option<u32>,
    string_ends_with: Option<u32>,
    string_eq: Option<u32>,
    string_not_eq: Option<u32>,
    string_less: Option<u32>,
    string_less_or_equal: Option<u32>,
    string_greater: Option<u32>,
    string_greater_or_equal: Option<u32>,
    string_is_empty: Option<u32>,
    string_is_ascii: Option<u32>,
    string_is_ascii_digit: Option<u32>,
    string_is_ascii_hex_digit: Option<u32>,
    string_is_ascii_alpha: Option<u32>,
    string_is_ascii_lower: Option<u32>,
    string_is_ascii_upper: Option<u32>,
    string_is_ascii_alnum: Option<u32>,
    string_is_ascii_identifier: Option<u32>,
    string_is_ascii_whitespace: Option<u32>,
    string_find_byte: Option<u32>,
    string_last_find_byte: Option<u32>,
    string_index_of: Option<u32>,
    string_last_index_of: Option<u32>,
    string_count: Option<u32>,
    string_parse_int: Option<u32>,
}

fn emit_string_concat_helper(code: &mut Vec<u8>, layout: &Layout, string_len_rva: u32) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    code.extend_from_slice(&[0x49, 0x89, 0xd3]);
    code.extend_from_slice(&[0x4c, 0x89, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0x4c, 0x89, 0x5c, 0x24, 0x28]);
    code.extend_from_slice(&[0x4c, 0x89, 0xd1]);
    emit_direct_call(code, layout.text_rva, string_len_rva);
    code.extend_from_slice(&[0x49, 0x89, 0xc0]);
    code.extend_from_slice(&[0x4c, 0x89, 0xd9]);
    emit_direct_call(code, layout.text_rva, string_len_rva);
    code.extend_from_slice(&[0x4c, 0x01, 0xc0]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x40, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x5c, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    code.extend_from_slice(&[0x49, 0x89, 0xc1]);
    code.extend_from_slice(&[0x4d, 0x89, 0xd0]);
    let left_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x00]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let left_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x88, 0x01]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_short_jump_back(code, left_loop);
    let right_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x03]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let right_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x88, 0x01]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_short_jump_back(code, right_loop);
    let done = code.len();
    code.extend_from_slice(&[0x41, 0xc6, 0x01, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0xd0]);
    code.push(0xc3);
    let allocation_failed_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]);
    code.push(0xc3);

    patch_short_jump(code, left_done, right_loop);
    patch_short_jump(code, right_done, done);
    patch_short_jump(code, allocation_failed, allocation_failed_target);
}

fn emit_string_len_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc0]);
    code.extend_from_slice(&[0x80, 0x3c, 0x01, 0x00]);
    code.extend_from_slice(&[0x74, 0x05]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf5]);
    code.push(0xc3);
}

fn emit_string_utf8_len_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x89, 0xca, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x8a, 0x02, 0x45, 0x84, 0xc0]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xe0, 0xc0, 0x41, 0x80, 0xf8, 0x80]);
    let continuation = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    let next = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let done_target = code.len();
    code.push(0xc3);
    let null_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_value, null_target);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, continuation, next);
}

fn emit_string_utf8_valid_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_value = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x08, 0x45, 0x84, 0xc9]);
    let done = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x80]);
    let ascii = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xc2]);
    let invalid_lead = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let two = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf0]);
    let three = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf5]);
    let four_invalid = emit_near_jump_placeholder(code, 0x87);

    code.extend_from_slice(&[
        0x45, 0x8a, 0x50, 0x01, 0x45, 0x8a, 0x58, 0x02, 0x41, 0x8a, 0x4c, 0x20, 0x03,
    ]);
    let four_second_checks = emit_utf8_continuation_check_pe(code, 0x41, 0xfa);
    let four_third_checks = emit_utf8_continuation_check_pe(code, 0x41, 0xfb);
    let four_fourth_checks = emit_utf8_continuation_check_pe(code, 0x00, 0xf9);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x90]);
    let four_second_low = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf4]);
    let four_not_f4 = emit_near_jump_placeholder(code, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x8f]);
    let four_second_high = emit_near_jump_placeholder(code, 0x87);
    let four_advance = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x04]);
    let four_loop = emit_near_unconditional_placeholder_pe(code);

    let three_target = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x50, 0x01, 0x45, 0x8a, 0x58, 0x02]);
    let three_second_checks = emit_utf8_continuation_check_pe(code, 0x41, 0xfa);
    let three_third_checks = emit_utf8_continuation_check_pe(code, 0x41, 0xfb);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let three_not_e0 = emit_near_jump_placeholder(code, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xa0]);
    let three_e0_low = emit_near_jump_placeholder(code, 0x82);
    let three_e0_done = emit_near_unconditional_placeholder_pe(code);
    let three_ed_check = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xed]);
    let three_not_ed = emit_near_jump_placeholder(code, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x9f]);
    let three_ed_high = emit_near_jump_placeholder(code, 0x87);
    let three_advance = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x03]);
    let three_loop = emit_near_unconditional_placeholder_pe(code);

    let two_target = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x50, 0x01]);
    let two_checks = emit_utf8_continuation_check_pe(code, 0x41, 0xfa);
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x02]);
    let two_loop = emit_near_unconditional_placeholder_pe(code);

    let ascii_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    let ascii_loop = emit_near_unconditional_placeholder_pe(code);

    let success = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);

    patch_near_jump(code, null_value, success);
    patch_near_jump(code, done, success);
    patch_near_jump(code, ascii, ascii_target);
    patch_near_jump(code, invalid_lead, failure);
    patch_near_jump(code, two, two_target);
    patch_near_jump(code, three, three_target);
    patch_near_jump(code, four_invalid, failure);
    patch_near_jump(code, four_second_low, failure);
    patch_near_jump(code, four_not_f4, four_advance);
    patch_near_jump(code, four_second_high, failure);
    patch_near_jump(code, three_not_e0, three_ed_check);
    patch_near_jump(code, three_e0_low, failure);
    patch_near_jump(code, three_e0_done, three_advance);
    patch_near_jump(code, three_not_ed, three_advance);
    patch_near_jump(code, three_ed_high, failure);
    patch_near_jump(code, four_loop, loop_start);
    patch_near_jump(code, three_loop, loop_start);
    patch_near_jump(code, two_loop, loop_start);
    patch_near_jump(code, ascii_loop, loop_start);
    for displacement in four_second_checks
        .into_iter()
        .chain(four_third_checks)
        .chain(four_fourth_checks)
        .chain(three_second_checks)
        .chain(three_third_checks)
        .chain(two_checks)
    {
        patch_near_jump(code, displacement, failure);
    }
}

fn emit_utf8_continuation_check_pe(code: &mut Vec<u8>, rex: u8, modrm: u8) -> [usize; 2] {
    if rex != 0 {
        code.extend_from_slice(&[rex, 0x80, modrm, 0x80]);
    } else {
        code.extend_from_slice(&[0x80, modrm, 0x80]);
    }
    let below = emit_near_jump_placeholder(code, 0x82);
    if rex != 0 {
        code.extend_from_slice(&[rex, 0x80, modrm, 0xbf]);
    } else {
        code.extend_from_slice(&[0x80, modrm, 0xbf]);
    }
    let above = emit_near_jump_placeholder(code, 0x87);
    [below, above]
}

fn emit_near_unconditional_placeholder_pe(code: &mut Vec<u8>) -> usize {
    code.push(0xe9);
    let displacement = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    displacement
}

#[derive(Clone, Copy)]
enum Utf8NavigationKindPe {
    ByteOffset,
    NextOffset,
    PrevOffset,
    IndexAt,
    IsBoundary,
}

fn emit_string_utf8_navigation_helper(code: &mut Vec<u8>, kind: Utf8NavigationKindPe) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_value = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x41, 0x54]);
    code.extend_from_slice(&[
        0x49, 0x89, 0xc8, 0x48, 0x89, 0xd6, 0x31, 0xc0, 0x45, 0x31, 0xd2, 0x45, 0x31, 0xe4,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x08, 0x45, 0x84, 0xc9]);
    let end = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x80]);
    let width_one = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let width_two = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf0]);
    let width_three = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf5]);
    let invalid = emit_near_jump_placeholder(code, 0x83);
    code.extend_from_slice(&[0x41, 0xbb, 0x04, 0, 0, 0]);
    let width_four_loop = emit_near_unconditional_placeholder_pe(code);
    let width_three_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x03, 0, 0, 0]);
    let width_three_loop = emit_near_unconditional_placeholder_pe(code);
    let width_two_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x02, 0, 0, 0]);
    let width_two_loop = emit_near_unconditional_placeholder_pe(code);
    let width_one_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x01, 0, 0, 0]);
    let width_one_loop = emit_near_unconditional_placeholder_pe(code);
    let width_ready = code.len();
    let boundary = if matches!(kind, Utf8NavigationKindPe::ByteOffset) {
        None
    } else {
        code.extend_from_slice(&[0x48, 0x39, 0xf0]);
        Some(emit_near_jump_placeholder(code, 0x84))
    };
    let byte_boundary = if matches!(kind, Utf8NavigationKindPe::ByteOffset) {
        code.extend_from_slice(&[0x4c, 0x39, 0xe6]);
        Some(emit_near_jump_placeholder(code, 0x84))
    } else {
        None
    };
    let inside = if matches!(kind, Utf8NavigationKindPe::ByteOffset) {
        None
    } else {
        code.extend_from_slice(&[0x48, 0x89, 0xc2, 0x4c, 0x01, 0xda, 0x48, 0x39, 0xd6]);
        Some(emit_near_jump_placeholder(code, 0x82))
    };
    code.extend_from_slice(&[
        0x49, 0x89, 0xc2, 0x4c, 0x01, 0xd8, 0x4d, 0x01, 0xd8, 0x49, 0xff, 0xc4,
    ]);
    let advance = emit_near_unconditional_placeholder_pe(code);
    let boundary_target = code.len();
    emit_navigation_result_pe(code, kind, true);
    let end_target = code.len();
    if matches!(kind, Utf8NavigationKindPe::ByteOffset) {
        code.extend_from_slice(&[0x4c, 0x39, 0xe6]);
    } else {
        code.extend_from_slice(&[0x48, 0x39, 0xf0]);
    }
    let end_match = emit_near_jump_placeholder(code, 0x84);
    let failure = code.len();
    emit_navigation_failure_pe(code, kind);
    let success = code.len();
    emit_navigation_result_pe(code, kind, false);
    patch_near_jump(code, null_value, failure);
    patch_near_jump(code, end, end_target);
    patch_near_jump(code, end_match, success);
    patch_near_jump(code, invalid, failure);
    patch_near_jump(code, advance, loop_start);
    patch_near_jump(code, width_one, width_one_target);
    patch_near_jump(code, width_two, width_two_target);
    patch_near_jump(code, width_three, width_three_target);
    patch_near_jump(code, width_four_loop, width_ready);
    patch_near_jump(code, width_three_loop, width_ready);
    patch_near_jump(code, width_two_loop, width_ready);
    patch_near_jump(code, width_one_loop, width_ready);
    if let Some(jump) = boundary {
        patch_near_jump(code, jump, boundary_target);
    }
    if let Some(jump) = byte_boundary {
        patch_near_jump(code, jump, boundary_target);
    }
    if let Some(jump) = inside {
        patch_near_jump(code, jump, failure);
    }
}

fn emit_navigation_result_pe(code: &mut Vec<u8>, kind: Utf8NavigationKindPe, at_boundary: bool) {
    match kind {
        Utf8NavigationKindPe::ByteOffset => {
            code.extend_from_slice(&[0x48, 0x89, 0xc0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKindPe::NextOffset if at_boundary => {
            code.extend_from_slice(&[0x4c, 0x01, 0xd8, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKindPe::NextOffset => {
            code.extend_from_slice(&[0x48, 0x89, 0xc0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKindPe::PrevOffset => {
            code.extend_from_slice(&[0x4c, 0x89, 0xd0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKindPe::IndexAt => {
            code.extend_from_slice(&[0x4c, 0x89, 0xe0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKindPe::IsBoundary => {
            code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0x41, 0x5c, 0xc3])
        }
    }
}

fn emit_navigation_failure_pe(code: &mut Vec<u8>, kind: Utf8NavigationKindPe) {
    if matches!(kind, Utf8NavigationKindPe::IsBoundary) {
        code.extend_from_slice(&[0x31, 0xc0, 0x41, 0x5c, 0xc3]);
    } else {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x41, 0x5c, 0xc3]);
    }
}

fn emit_string_utf8_codepoint_at_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_value = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xc8, 0x45, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x10, 0x45, 0x84, 0xd2]);
    let end = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0x39, 0xd1]);
    let skip = emit_near_jump_placeholder(code, 0x85);

    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x80]);
    let ascii = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xe0]);
    let two = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xf0]);
    let three = emit_near_jump_placeholder(code, 0x82);

    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x07, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x02,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x03,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);
    let ascii_target = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0xc2, 0xc3]);
    let two_target = code.len();
    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x1f, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);
    let three_target = code.len();
    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x0f, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x02,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);

    let skip_target = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x80]);
    let advance_one = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xe0]);
    let advance_two = emit_near_jump_placeholder(code, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xf0]);
    let advance_three = emit_near_jump_placeholder(code, 0x82);
    let advance_four = emit_codepoint_advance_and_loop(code, 4);
    let advance_three_target = code.len();
    let back_three = emit_codepoint_advance_and_loop(code, 3);
    let advance_two_target = code.len();
    let back_two = emit_codepoint_advance_and_loop(code, 2);
    let advance_one_target = code.len();
    let back_one = emit_codepoint_advance_and_loop(code, 1);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    patch_near_jump(code, null_value, failure);
    patch_near_jump(code, end, failure);
    patch_near_jump(code, skip, skip_target);
    patch_near_jump(code, ascii, ascii_target);
    patch_near_jump(code, two, two_target);
    patch_near_jump(code, three, three_target);
    patch_near_jump(code, advance_one, advance_one_target);
    patch_near_jump(code, advance_two, advance_two_target);
    patch_near_jump(code, advance_three, advance_three_target);
    patch_near_jump(code, advance_four, loop_start);
    patch_near_jump(code, back_one, loop_start);
    patch_near_jump(code, back_two, loop_start);
    patch_near_jump(code, back_three, loop_start);
}

fn emit_codepoint_advance_and_loop(code: &mut Vec<u8>, amount: u8) -> usize {
    code.extend_from_slice(&[0x49, 0x83, 0xc0, amount, 0x49, 0xff, 0xc1, 0x45, 0x39, 0xc9]);
    emit_near_jump_placeholder(code, 0x84)
}

fn emit_array_new_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x4c, 0x24, 0x28, 0x48, 0x89, 0x54, 0x24, 0x30, 0x48,
        0x0f, 0xaf, 0xca, 0x48, 0x83, 0xc1, 0x20, 0x48, 0x89, 0xca, 0x31, 0xc9, 0x41, 0xb8, 0x00,
        0x30, 0x00, 0x00, 0x41, 0xb9, 0x04, 0x00, 0x00, 0x00,
    ]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x38, 0x48, 0x8d, 0x50, 0x20, 0x48, 0x89, 0x10, 0x48, 0xc7, 0x40,
        0x08, 0x00, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x4c, 0x24, 0x30, 0x48, 0x89, 0x48, 0x10, 0x48,
        0x8b, 0x4c, 0x24, 0x28, 0x48, 0x89, 0x48, 0x18, 0x48, 0x8b, 0x44, 0x24, 0x38, 0x48, 0x83,
        0xc4, 0x48, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_near_jump(code, failed, failure);
}

fn emit_array_clone_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x58, 0x48, 0x89, 0x4c, 0x24, 0x40, 0x48, 0x85, 0xc9,
    ]);
    let null = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x41, 0x18, 0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x41, 0x10, 0x48, 0x89,
        0x44, 0x24, 0x30, 0x48, 0x8b, 0x41, 0x08, 0x48, 0x89, 0x44, 0x24, 0x38, 0x48, 0x8b, 0x44,
        0x24, 0x30, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x28, 0x48, 0x83, 0xc0, 0x20, 0x48, 0x89, 0xc2,
        0x31, 0xc9, 0x41, 0xb8, 0x00, 0x30, 0x00, 0x00, 0x41, 0xb9, 0x04, 0x00, 0x00, 0x00,
    ]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x48, 0x48, 0x8b, 0x4c, 0x24, 0x48, 0x48, 0x8d, 0x50, 0x20, 0x48,
        0x89, 0x10, 0x48, 0x8b, 0x44, 0x24, 0x38, 0x48, 0x89, 0x41, 0x08, 0x48, 0x8b, 0x44, 0x24,
        0x30, 0x48, 0x89, 0x41, 0x10, 0x48, 0x8b, 0x44, 0x24, 0x28, 0x48, 0x89, 0x41, 0x18, 0x48,
        0x8b, 0x44, 0x24, 0x38, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x54, 0x24, 0x48,
        0x4c, 0x8b, 0x12, 0x48, 0x8b, 0x4c, 0x24, 0x40, 0x4c, 0x8b, 0x19,
    ]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let copy_done = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x45, 0x8a, 0x0b, 0x45, 0x88, 0x0a, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc3, 0x48, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, copy_loop);
    let done = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_near_jump(code, null, failure);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_conditional(code, copy_done, done);
}

fn emit_array_reserve_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x60, 0x48, 0x89, 0x4c, 0x24, 0x40, 0x48, 0x89, 0x54, 0x24, 0x48, 0x48,
        0x85, 0xc9,
    ]);
    let null = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x3b, 0x51, 0x10]);
    let already_large = emit_near_conditional_placeholder(code, 0x86);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x41, 0x18, 0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x41, 0x08, 0x48, 0x89,
        0x44, 0x24, 0x38, 0x48, 0x8b, 0x44, 0x24, 0x48, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x28, 0x48,
        0x83, 0xc0, 0x20, 0x48, 0x89, 0xc2, 0x31, 0xc9, 0x41, 0xb8, 0x00, 0x30, 0x00, 0x00, 0x41,
        0xb9, 0x04, 0x00, 0x00, 0x00,
    ]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x50, 0x48, 0x8b, 0x4c, 0x24, 0x50, 0x48, 0x8d, 0x50, 0x20, 0x48,
        0x89, 0x10, 0x48, 0x8b, 0x44, 0x24, 0x38, 0x48, 0x89, 0x41, 0x08, 0x48, 0x8b, 0x44, 0x24,
        0x48, 0x48, 0x89, 0x41, 0x10, 0x48, 0x8b, 0x44, 0x24, 0x28, 0x48, 0x89, 0x41, 0x18, 0x48,
        0x8b, 0x44, 0x24, 0x38, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x54, 0x24, 0x50,
        0x4c, 0x8b, 0x12, 0x48, 0x8b, 0x4c, 0x24, 0x40, 0x4c, 0x8b, 0x19,
    ]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let copy_done = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[
        0x45, 0x8a, 0x0b, 0x45, 0x88, 0x0a, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc3, 0x48, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, copy_loop);
    let free_old = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4c, 0x24, 0x40, 0x31, 0xd2, 0x41, 0xb8, 0x00, 0x80, 0x00, 0x00,
    ]);
    emit_call_iat(code, layout, layout.virtual_free_iat);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x50, 0x48, 0x83, 0xc4, 0x60, 0xc3]);
    let return_source = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x60, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x60, 0xc3]);
    patch_near_jump(code, null, failure);
    patch_near_conditional(code, already_large, return_source);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_conditional(code, copy_done, free_old);
}

fn emit_array_clear_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0xc7, 0x41, 0x08, 0x00, 0x00, 0x00, 0x00, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
}

fn emit_array_free_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28, 0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xd2, 0x41, 0xb8, 0x00, 0x80, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_free_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_first_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x83, 0x79, 0x08, 0x00]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x01, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
}

fn emit_array_last_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc8, 0x48, 0x8b, 0x09, 0x48, 0x01, 0xc8, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
}

fn emit_array_fill_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x45, 0x8a, 0x1a, 0x48, 0x8b, 0x41, 0x08, 0x4c, 0x8b, 0x01]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x45, 0x88, 0x18, 0x49, 0xff, 0xc0, 0x48, 0xff, 0xc8]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, done, success);
}

fn emit_array_reverse_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc8, 0x4c, 0x8b, 0x01, 0x4d, 0x8d, 0x0c, 0x00]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x4d, 0x39, 0xc8]);
    let done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x45, 0x8a, 0x10, 0x45, 0x8a, 0x19, 0x45, 0x88, 0x18, 0x45, 0x88, 0x11, 0x49, 0xff, 0xc0,
        0x49, 0xff, 0xc9,
    ]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, success);
    patch_short_jump(code, done, success);
}

fn emit_array_index_helper(code: &mut Vec<u8>, last: bool, count: bool) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x45, 0x8a, 0x1a, 0x4c, 0x8b, 0x49, 0x08, 0x4c, 0x8b, 0x01, 0x45, 0x31, 0xd2,
    ]);
    if last {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    } else {
        code.extend_from_slice(&[0x31, 0xc0]);
    }
    let loop_start = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xc9]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x45, 0x38, 0x18]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc9]);
    emit_short_jump_back(code, loop_start);
    let match_target = code.len();
    if count {
        code.extend_from_slice(&[
            0x48, 0xff, 0xc0, 0x49, 0xff, 0xc0, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc9,
        ]);
        emit_short_jump_back(code, loop_start);
    } else if last {
        code.extend_from_slice(&[
            0x4c, 0x89, 0xd8, 0x49, 0xff, 0xc0, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc9,
        ]);
        emit_short_jump_back(code, loop_start);
    } else {
        code.extend_from_slice(&[0x4c, 0x89, 0xd8, 0xc3]);
    }
    let result_done = if last || count {
        let target = code.len();
        code.push(0xc3);
        Some(target)
    } else {
        None
    };
    let failure = code.len();
    if count {
        code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    } else {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    }
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, done, result_done.unwrap_or(failure));
    patch_short_jump(code, matched, match_target);
}

fn emit_array_len_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0xc3]);
    let zero = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, zero);
}

fn emit_array_capacity_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x10, 0xc3]);
    let zero = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, zero);
}

fn emit_array_is_empty_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x83, 0x79, 0x08, 0x00]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let empty = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, empty);
    patch_short_jump(code, done, empty);
}

fn emit_array_get_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0x48, 0x39, 0xc2]);
    let failed = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x4c, 0x8b, 0x49, 0x18, 0x4c, 0x0f, 0xaf, 0xca, 0x48, 0x8b, 0x01, 0x4a, 0x8d, 0x04, 0x08,
        0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_set_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0x48, 0x39, 0xc2]);
    let failed = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x4c, 0x8b, 0x49, 0x18, 0x4c, 0x0f, 0xaf, 0xca, 0x48, 0x8b, 0x01, 0x4c, 0x01, 0xc8, 0x41,
        0x8a, 0x08, 0x88, 0x08, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_push_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x41, 0x08, 0x48, 0x3b, 0x41, 0x10]);
    let full = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x4c, 0x8b, 0x49, 0x18, 0x4c, 0x0f, 0xaf, 0xc8, 0x4c, 0x8b, 0x01, 0x4d, 0x01, 0xc8, 0x44,
        0x8a, 0x0a, 0x45, 0x88, 0x08, 0x48, 0xff, 0x41, 0x08, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, full, failure);
}

fn emit_string_byte_at_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    code.extend_from_slice(&[0x49, 0x39, 0xd0]);
    code.extend_from_slice(&[0x74, 0x0c]);
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x01, 0x00]);
    code.extend_from_slice(&[0x74, 0x0f]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xef]);
    code.extend_from_slice(&[0x42, 0x0f, 0xb6, 0x04, 0x01]);
    code.extend_from_slice(&[0x84, 0xc0]);
    code.extend_from_slice(&[0x74, 0x01]);
    code.push(0xc3);
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.push(0xc3);
}

fn emit_string_compare_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    code.extend_from_slice(&[0x75, 0x0e]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    code.extend_from_slice(&[0x74, 0x07]);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    code.extend_from_slice(&[0xeb, 0xe7]);
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);
    code.extend_from_slice(&[0x44, 0x89, 0xc0]);
    code.extend_from_slice(&[0x44, 0x29, 0xc8]);
    code.push(0xc3);
}

fn emit_string_contains_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    let outer = code.len();
    code.extend_from_slice(&[0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0x38, 0x00]);
    let haystack_end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4d, 0x89, 0xc1]);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    let inner = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0x39, 0x00]);
    let advance = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x3a, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    emit_short_jump_back(code, inner);
    let advance_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, outer);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty_needle, true_target);
    patch_short_jump(code, matched, true_target);
    patch_short_jump(code, haystack_end, false_target);
    patch_short_jump(code, advance, advance_target);
    patch_short_jump(code, mismatch, advance_target);
}

fn emit_string_starts_with_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let prefix_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let source_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x38, 0xc0]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, prefix_done, true_target);
    patch_short_jump(code, source_done, false_target);
    patch_short_jump(code, mismatch, false_target);
}

fn emit_string_ends_with_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    let source_len_loop = code.len();
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x01, 0x00]);
    let source_len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, source_len_loop);
    let source_len_done_target = code.len();
    code.extend_from_slice(&[0x4d, 0x31, 0xc9]);
    let suffix_len_loop = code.len();
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x0a, 0x00]);
    let suffix_len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_short_jump_back(code, suffix_len_loop);
    let suffix_len_done_target = code.len();
    code.extend_from_slice(&[0x4d, 0x39, 0xc8]);
    let suffix_too_long = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x4c, 0x01, 0xc1]);
    code.extend_from_slice(&[0x4c, 0x29, 0xc9]);
    let compare_loop = code.len();
    code.extend_from_slice(&[0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x38, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, compare_loop);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, source_len_done, source_len_done_target);
    patch_short_jump(code, suffix_len_done, suffix_len_done_target);
    patch_short_jump(code, suffix_too_long, false_target);
    patch_short_jump(code, matched, true_target);
    patch_short_jump(code, mismatch, false_target);
}

fn emit_string_eq_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, mismatch, false_target);
    patch_short_jump(code, matched, true_target);
}

fn emit_string_not_eq_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);

    patch_short_jump(code, mismatch, true_target);
    patch_short_jump(code, matched, false_target);
}

fn emit_string_less_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let difference = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);
    let difference_target = code.len();
    code.extend_from_slice(&[0x0f, 0x92, 0xc0, 0x0f, 0xb6, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, difference, difference_target);
    patch_short_jump(code, matched, false_target);
}

fn emit_string_less_or_equal_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let difference = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let difference_target = code.len();
    code.extend_from_slice(&[0x0f, 0x96, 0xc0, 0x0f, 0xb6, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, difference, difference_target);
    patch_short_jump(code, matched, true_target);
}

fn emit_string_greater_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let difference = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);
    let difference_target = code.len();
    code.extend_from_slice(&[0x0f, 0x97, 0xc0, 0x0f, 0xb6, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, difference, difference_target);
    patch_short_jump(code, matched, false_target);
}

fn emit_string_greater_or_equal_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x44, 0x0f, 0xb6, 0x0a]);
    code.extend_from_slice(&[0x45, 0x39, 0xc8]);
    let difference = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let difference_target = code.len();
    code.extend_from_slice(&[0x0f, 0x93, 0xc0, 0x0f, 0xb6, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, difference, difference_target);
    patch_short_jump(code, matched, true_target);
}

fn emit_string_is_empty_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x01]);
    code.extend_from_slice(&[0x85, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x94, 0xc0]);
    code.push(0xc3);
}

fn emit_string_is_ascii_helper(code: &mut Vec<u8>) {
    let loop_start = code.len();
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, 0x7f]);
    let non_ascii = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, end, true_target);
    patch_short_jump(code, non_ascii, false_target);
}

fn emit_string_is_ascii_digit_helper(code: &mut Vec<u8>) {
    emit_string_all_bytes_in_range_helper(code, b'0', b'9');
}

fn emit_string_is_ascii_hex_digit_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'0']);
    let below_digit = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'9']);
    let accepted_digit = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'A']);
    let below_upper = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'F']);
    let accepted_upper = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'a']);
    let below_lower = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'f']);
    let above_lower = emit_short_jump_placeholder(code, 0x77);
    let accepted_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let next = emit_short_jump_placeholder(code, 0x75);
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, below_digit, false_target);
    patch_short_jump(code, accepted_digit, accepted_target);
    patch_short_jump(code, below_upper, false_target);
    patch_short_jump(code, accepted_upper, accepted_target);
    patch_short_jump(code, below_lower, false_target);
    patch_short_jump(code, above_lower, false_target);
    patch_short_jump(code, next, loop_start);
}

fn emit_string_is_ascii_alpha_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'A']);
    let below_upper = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'Z']);
    let accepted_upper = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'a']);
    let below_lower = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'z']);
    let above_lower = emit_short_jump_placeholder(code, 0x77);
    let accepted_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let next = emit_short_jump_placeholder(code, 0x75);
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, below_upper, false_target);
    patch_short_jump(code, accepted_upper, accepted_target);
    patch_short_jump(code, below_lower, false_target);
    patch_short_jump(code, above_lower, false_target);
    patch_short_jump(code, next, loop_start);
}

fn emit_string_is_ascii_lower_helper(code: &mut Vec<u8>) {
    emit_string_all_bytes_in_range_helper(code, b'a', b'z');
}

fn emit_string_is_ascii_upper_helper(code: &mut Vec<u8>) {
    emit_string_all_bytes_in_range_helper(code, b'A', b'Z');
}

fn emit_string_is_ascii_alnum_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'0']);
    let below_digit = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'9']);
    let accepted_digit = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'A']);
    let below_upper = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'Z']);
    let accepted_upper = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'a']);
    let below_lower = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'z']);
    let above_lower = emit_short_jump_placeholder(code, 0x77);
    let accepted_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let next = emit_short_jump_placeholder(code, 0x75);
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, below_digit, false_target);
    patch_short_jump(code, accepted_digit, accepted_target);
    patch_short_jump(code, below_upper, false_target);
    patch_short_jump(code, accepted_upper, accepted_target);
    patch_short_jump(code, below_lower, false_target);
    patch_short_jump(code, above_lower, false_target);
    patch_short_jump(code, next, loop_start);
}

fn emit_string_is_ascii_identifier_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'_']);
    let first_underscore = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'A']);
    let first_below_upper = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'Z']);
    let first_upper = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'a']);
    let first_below_lower = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'z']);
    let first_above_lower = emit_short_jump_placeholder(code, 0x77);
    let accepted_first_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let first_was_only_byte = emit_short_jump_placeholder(code, 0x74);
    let rest_loop = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'_']);
    let rest_underscore = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'0']);
    let rest_below_digit = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'9']);
    let rest_digit = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'A']);
    let rest_below_upper = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'Z']);
    let rest_upper = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'a']);
    let rest_below_lower = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'z']);
    let rest_above_lower = emit_short_jump_placeholder(code, 0x77);
    let accepted_rest_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let rest_next = emit_short_jump_placeholder(code, 0x75);
    let true_target = code.len();
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, first_underscore, accepted_first_target);
    patch_short_jump(code, first_below_upper, false_target);
    patch_short_jump(code, first_upper, accepted_first_target);
    patch_short_jump(code, first_below_lower, false_target);
    patch_short_jump(code, first_above_lower, false_target);
    patch_short_jump(code, first_was_only_byte, true_target);
    patch_short_jump(code, rest_underscore, accepted_rest_target);
    patch_short_jump(code, rest_below_digit, false_target);
    patch_short_jump(code, rest_digit, accepted_rest_target);
    patch_short_jump(code, rest_below_upper, false_target);
    patch_short_jump(code, rest_upper, accepted_rest_target);
    patch_short_jump(code, rest_below_lower, false_target);
    patch_short_jump(code, rest_above_lower, false_target);
    patch_short_jump(code, rest_next, rest_loop);
}

fn emit_string_is_ascii_whitespace_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b' ']);
    let accepted_space = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'\t']);
    let below_tab = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, b'\r']);
    let above_cr = emit_short_jump_placeholder(code, 0x77);
    let accepted_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let next = emit_short_jump_placeholder(code, 0x75);
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, accepted_space, accepted_target);
    patch_short_jump(code, below_tab, false_target);
    patch_short_jump(code, above_cr, false_target);
    patch_short_jump(code, next, loop_start);
}

fn emit_string_find_byte_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x83, 0xfa, 0x00]);
    let below_zero = emit_short_jump_placeholder(code, 0x7c);
    code.extend_from_slice(&[0x81, 0xfa, 0xff, 0x00, 0x00, 0x00]);
    let above_byte = emit_short_jump_placeholder(code, 0x7f);
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x42, 0x8a, 0x04, 0x01]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x38, 0xd0]);
    let found = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, loop_start);
    let found_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xc0]);
    code.push(0xc3);
    let not_found_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.push(0xc3);

    patch_short_jump(code, below_zero, not_found_target);
    patch_short_jump(code, above_byte, not_found_target);
    patch_short_jump(code, end, not_found_target);
    patch_short_jump(code, found, found_target);
}

fn emit_string_last_find_byte_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x83, 0xfa, 0x00]);
    let below_zero = emit_short_jump_placeholder(code, 0x7c);
    code.extend_from_slice(&[0x81, 0xfa, 0xff, 0x00, 0x00, 0x00]);
    let above_byte = emit_short_jump_placeholder(code, 0x7f);
    code.extend_from_slice(&[0x49, 0xc7, 0xc1, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x42, 0x8a, 0x04, 0x01]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x38, 0xd0]);
    let no_match = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x4d, 0x89, 0xc1]);
    let advance_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, loop_start);
    let end_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xc8, 0xc3]);

    patch_short_jump(code, below_zero, end_target);
    patch_short_jump(code, above_byte, end_target);
    patch_short_jump(code, end, end_target);
    patch_short_jump(code, no_match, advance_target);
}

fn emit_string_index_of_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    code.extend_from_slice(&[0x4d, 0x31, 0xdb]);
    code.extend_from_slice(&[0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    let outer = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0x38, 0x00]);
    let haystack_end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4d, 0x89, 0xc1]);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    let inner = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0x39, 0x00]);
    let source_ended = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x3a, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    emit_short_jump_back(code, inner);
    let advance_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    emit_short_jump_back(code, outer);
    let found_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xd8]);
    code.push(0xc3);
    let not_found_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.push(0xc3);

    patch_short_jump(code, empty_needle, found_target);
    patch_short_jump(code, matched, found_target);
    patch_short_jump(code, haystack_end, not_found_target);
    patch_short_jump(code, source_ended, not_found_target);
    patch_short_jump(code, mismatch, advance_target);
}

fn emit_string_last_index_of_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc3, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x80, 0x3a, 0x00]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    let outer = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0x3c, 0x08, 0x00]);
    let source_end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4d, 0x8d, 0x0c, 0x08]);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    let inner = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0x39, 0x00]);
    let advance = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x3a, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    emit_short_jump_back(code, inner);
    let found_target = code.len();
    code.extend_from_slice(&[0x49, 0x89, 0xcb, 0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let advance_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let return_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xd8]);
    code.push(0xc3);

    patch_short_jump(code, empty_needle, return_target);
    patch_short_jump(code, source_end, return_target);
    patch_short_jump(code, matched, found_target);
    patch_short_jump(code, advance, advance_target);
    patch_short_jump(code, mismatch, advance_target);
}

fn emit_string_count_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    code.extend_from_slice(&[0x4d, 0x31, 0xc9]);
    code.extend_from_slice(&[0x80, 0x3a, 0x00]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    let outer = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0x38, 0x00]);
    let source_end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4d, 0x89, 0xc3]);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    let inner = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x02]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x80, 0x3b, 0x00]);
    let advance = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x3a, 0x03]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    emit_short_jump_back(code, inner);
    let found_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x4d, 0x89, 0xd8]);
    emit_short_jump_back(code, outer);
    let advance_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, outer);
    let return_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xc8]);
    code.push(0xc3);

    patch_short_jump(code, empty_needle, return_target);
    patch_short_jump(code, source_end, return_target);
    patch_short_jump(code, matched, found_target);
    patch_short_jump(code, advance, return_target);
    patch_short_jump(code, mismatch, advance_target);
}

fn emit_string_parse_int_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    code.extend_from_slice(&[0x49, 0x31, 0xc9]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc2, 0x01, 0x00, 0x00, 0x00]);

    let skip_whitespace = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x00]);
    let mut whitespace_jumps = Vec::new();
    for byte in [b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c] {
        code.extend_from_slice(&[0x3c, byte]);
        whitespace_jumps.push(emit_short_jump_placeholder(code, 0x74));
    }

    code.extend_from_slice(&[0x3c, b'-']);
    let positive_sign = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xc7, 0xc2, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    let digit_start = emit_short_jump_placeholder(code, 0xeb);

    let positive_sign_target = code.len();
    code.extend_from_slice(&[0x3c, b'+']);
    let no_plus_sign = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    let plus_to_digits = emit_short_jump_placeholder(code, 0xeb);

    let digit_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x00]);
    code.extend_from_slice(&[0x3c, b'0']);
    let done_below = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x3c, b'9']);
    let done_above = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x4c, 0x0f, 0xb6, 0xd8]);
    code.extend_from_slice(&[0x49, 0x83, 0xeb, b'0']);
    code.extend_from_slice(&[0x4d, 0x6b, 0xc9, 0x0a]);
    code.extend_from_slice(&[0x4d, 0x01, 0xd9]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, digit_loop);

    let done = code.len();
    code.extend_from_slice(&[0x4d, 0x0f, 0xaf, 0xca]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc8]);
    code.push(0xc3);

    for jump in whitespace_jumps {
        patch_short_jump(code, jump, skip_whitespace);
    }
    patch_short_jump(code, positive_sign, positive_sign_target);
    patch_short_jump(code, digit_start, digit_loop);
    patch_short_jump(code, no_plus_sign, digit_loop);
    patch_short_jump(code, plus_to_digits, digit_loop);
    patch_short_jump(code, done_below, done);
    patch_short_jump(code, done_above, done);
}

fn emit_string_all_bytes_in_range_helper(code: &mut Vec<u8>, min: u8, max: u8) {
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf8, min]);
    let below_min = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x41, 0x80, 0xf8, max]);
    let above_max = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x44, 0x8a, 0x01]);
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let next = emit_short_jump_placeholder(code, 0x75);
    code.push(0xb8);
    code.extend_from_slice(&1_u32.to_le_bytes());
    code.push(0xc3);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, empty, false_target);
    patch_short_jump(code, below_min, false_target);
    patch_short_jump(code, above_max, false_target);
    patch_short_jump(code, next, loop_start);
}

fn emit_short_jump_placeholder(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.extend_from_slice(&[opcode, 0]);
    code.len() - 1
}

fn emit_near_jump_placeholder(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.extend_from_slice(&[0x0f, opcode, 0, 0, 0, 0]);
    code.len() - 4
}

fn patch_near_jump(code: &mut [u8], displacement_offset: usize, target: usize) {
    let next = displacement_offset + 4;
    let displacement = target as i64 - next as i64;
    code[displacement_offset..displacement_offset + 4]
        .copy_from_slice(&(displacement as i32).to_le_bytes());
}

fn emit_near_conditional_placeholder(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.extend_from_slice(&[0x0f, opcode]);
    let displacement = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    displacement
}

fn patch_near_conditional(code: &mut [u8], displacement_offset: usize, target: usize) {
    let next = displacement_offset + 4;
    let displacement = (target as isize - next as isize) as i32;
    code[displacement_offset..displacement_offset + 4].copy_from_slice(&displacement.to_le_bytes());
}

fn emit_short_jump_back(code: &mut Vec<u8>, target: usize) {
    code.push(0xeb);
    let displacement_offset = code.len();
    code.push(0);
    patch_short_jump(code, displacement_offset, target);
}

fn patch_short_jump(code: &mut [u8], displacement_offset: usize, target: usize) {
    let next = displacement_offset + 1;
    let displacement = target as isize - next as isize;
    debug_assert!((-128..=127).contains(&displacement));
    code[displacement_offset] = displacement as i8 as u8;
}

fn emit_bounds_check_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x39, 0xd1]);
    code.extend_from_slice(&[0x73, 0x01]);
    code.push(0xc3);
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.push(0xb9);
    code.extend_from_slice(&1_u32.to_le_bytes());
    emit_call_iat(code, layout, layout.exit_process_iat);
}

fn emit_print_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x28]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x28]);
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    code.extend_from_slice(&[0x43, 0x80, 0x3c, 0x02, 0x00]);
    code.extend_from_slice(&[0x74, 0x05]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf4]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc0]);
    code.extend_from_slice(&[0x4c, 0x89, 0x44, 0x24, 0x30]);
    code.push(0xb9);
    code.extend_from_slice(&(-11_i32).to_le_bytes());
    emit_call_iat(code, layout, layout.get_std_handle_iat);
    code.extend_from_slice(&[0x48, 0x89, 0xc1]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x28]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x44, 0x24, 0x30]);
    emit_lea(code, layout, &[0x4c, 0x8d, 0x0d], layout.written_rva);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]);
    emit_call_iat(code, layout, layout.write_file_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48]);
    code.push(0xc3);
}

fn emit_println_helper(code: &mut Vec<u8>, layout: &Layout, print_rva: u32, newline_rva: u32) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    emit_direct_call(code, layout.text_rva, print_rva);
    code.push(0xb9);
    code.extend_from_slice(&(-11_i32).to_le_bytes());
    emit_call_iat(code, layout, layout.get_std_handle_iat);
    code.extend_from_slice(&[0x48, 0x89, 0xc1]);
    emit_lea(code, layout, &[0x48, 0x8d, 0x15], newline_rva);
    code.extend_from_slice(&[0x41, 0xb8, 1, 0, 0, 0]);
    emit_lea(code, layout, &[0x4c, 0x8d, 0x0d], layout.written_rva);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]);
    emit_call_iat(code, layout, layout.write_file_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]);
    code.push(0xc3);
}

fn emit_process_exit_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    emit_call_iat(code, layout, layout.exit_process_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
}

fn emit_alloc_helper(code: &mut Vec<u8>, layout: &Layout, array: bool) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    if array {
        code.extend_from_slice(&[0x48, 0x0f, 0xaf, 0xca]);
    }
    code.extend_from_slice(&[0x48, 0x89, 0xca]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
}

fn emit_mem_copy_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    code.extend_from_slice(&[0x49, 0x89, 0xd3]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc0]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let done = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x13]);
    code.extend_from_slice(&[0x41, 0x88, 0x12]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x48, 0xff, 0xc8]);
    code.extend_from_slice(&[
        0x75,
        (loop_start as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    let done_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, done, done_target);
}

fn emit_mem_zero_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    code.extend_from_slice(&[0x48, 0x89, 0xd0]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let done = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x41, 0xc6, 0x02, 0x00]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x48, 0xff, 0xc8]);
    code.extend_from_slice(&[
        0x75,
        (loop_start as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    let done_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, done, done_target);
}

fn emit_mem_move_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x39, 0xd1]);
    let forward = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    code.extend_from_slice(&[0x4d, 0x01, 0xc2]);
    code.extend_from_slice(&[0x49, 0x89, 0xd3]);
    code.extend_from_slice(&[0x4d, 0x01, 0xc3]);
    code.extend_from_slice(&[0x49, 0xff, 0xca]);
    code.extend_from_slice(&[0x49, 0xff, 0xcb]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc0]);
    let backward_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x13]);
    code.extend_from_slice(&[0x41, 0x88, 0x12]);
    code.extend_from_slice(&[0x49, 0xff, 0xca]);
    code.extend_from_slice(&[0x49, 0xff, 0xcb]);
    code.extend_from_slice(&[0x48, 0xff, 0xc8]);
    code.extend_from_slice(&[
        0x75,
        (backward_loop as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let forward_target = code.len();
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    code.extend_from_slice(&[0x49, 0x89, 0xd3]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc0]);
    let forward_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x13]);
    code.extend_from_slice(&[0x41, 0x88, 0x12]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x48, 0xff, 0xc8]);
    code.extend_from_slice(&[
        0x75,
        (forward_loop as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, forward, forward_target);
}

fn emit_mem_fill_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xcf]);
    code.extend_from_slice(&[0x48, 0x89, 0xd1]);
    code.extend_from_slice(&[0x44, 0x88, 0xc0]);
    code.extend_from_slice(&[0xf3, 0xaa]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_mem_find_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x4d, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x46, 0x38, 0x04, 0x09]);
    let found = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let found_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xc8, 0xc3]);
    let not_found = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    patch_short_jump(code, empty, not_found);
    patch_short_jump(code, found, found_target);
}

fn emit_mem_compare_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x4d, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x01]);
    code.extend_from_slice(&[0x3a, 0x02]);
    let less = emit_short_jump_placeholder(code, 0x72);
    let greater = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc8]);
    emit_short_jump_back(code, loop_start);
    let equal_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let less_target = code.len();
    code.extend_from_slice(&[0xb8, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    let greater_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, empty, equal_target);
    patch_short_jump(code, less, less_target);
    patch_short_jump(code, greater, greater_target);
}

fn emit_mem_equal_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x4d, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x01]);
    code.extend_from_slice(&[0x3a, 0x02]);
    let not_equal = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc8]);
    emit_short_jump_back(code, loop_start);
    let equal_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, equal_target);
    patch_short_jump(code, not_equal, false_target);
}

fn emit_mem_is_zero_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x80, 0x39, 0x00]);
    let not_zero = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let zero_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, zero_target);
    patch_short_jump(code, not_zero, false_target);
}

fn emit_mem_reverse_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8d, 0x54, 0x11, 0xff]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x48, 0x39, 0xd1]);
    let done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[0x8a, 0x01]);
    code.extend_from_slice(&[0x44, 0x8a, 0x1a]);
    code.extend_from_slice(&[0x44, 0x88, 0x19]);
    code.extend_from_slice(&[0x88, 0x02]);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x48, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let done_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, done_target);
    patch_short_jump(code, done, done_target);
}

fn emit_string_from_byte_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x89, 0x4c, 0x24, 0x20]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0xba, 0x02, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x8a, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0x88, 0x10]);
    code.extend_from_slice(&[0xc6, 0x40, 0x01, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_short_jump(code, failed, failure);
}

fn emit_string_clone_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x89, 0xca]);
    code.extend_from_slice(&[0x48, 0x31, 0xc0]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x02, 0x00]);
    let length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, length_loop);
    let length_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x4d, 0x31, 0xdb]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x46, 0x8a, 0x0c, 0x1a]);
    code.extend_from_slice(&[0x47, 0x88, 0x0c, 0x1a]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x48, 0xff, 0xc9]);
    code.extend_from_slice(&[
        0x75,
        (copy_loop as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_short_jump(code, length_done, length_target);
    patch_short_jump(code, allocation_failed, failure);
}

fn emit_string_slice_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x78]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x4c, 0x89, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_source = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xca, 0x31, 0xc0]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0x3a, 0x00]);
    let length_done = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0xff, 0xc2, 0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, length_loop);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x58]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x39, 0xc2]);
    let start_out = emit_near_conditional_placeholder(code, 0x83);
    code.extend_from_slice(&[0x4c, 0x8b, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[0x4d, 0x85, 0xc0]);
    let zero_length = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x2b, 0xc2]);
    code.extend_from_slice(&[0x49, 0x39, 0xc0]);
    let clamp_length = emit_near_conditional_placeholder(code, 0x87);
    let clamp_target = code.len();
    code.extend_from_slice(&[0x49, 0x89, 0xc0]);
    let length_ready = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0x44, 0x24, 0x60]);
    code.extend_from_slice(&[0x4c, 0x89, 0xc2, 0x48, 0xff, 0xc2]);
    code.extend_from_slice(&[0x31, 0xc9, 0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x68]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x4c, 0x03, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x58, 0x68]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x4c, 0x24, 0x60]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xc9]);
    let copy_done = emit_near_conditional_placeholder(code, 0x84);
    code.extend_from_slice(&[0x41, 0x8a, 0x02, 0x41, 0x88, 0x03]);
    code.extend_from_slice(&[0x49, 0xff, 0xc2, 0x49, 0xff, 0xc3, 0x49, 0xff, 0xc9]);
    emit_short_jump_back(code, copy_loop);
    let done = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x68, 0x48, 0x83, 0xc4, 0x78, 0xc3]);
    let zero_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40, 0x48, 0x03, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x78, 0xc3]);
    let start_out_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40, 0x48, 0x03, 0x44, 0x24, 0x58]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x78, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x78, 0xc3]);

    patch_near_conditional(code, null_source, failure);
    patch_near_conditional(code, length_done, length_ready);
    patch_near_conditional(code, start_out, start_out_target);
    patch_near_conditional(code, zero_length, zero_target);
    patch_near_conditional(code, clamp_length, clamp_target);
    patch_near_conditional(code, allocation_failed, failure);
    patch_near_conditional(code, copy_done, done);
}

fn emit_alloc_copy_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x38]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x4d, 0x31, 0xdb]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x46, 0x8a, 0x0c, 0x1a]);
    code.extend_from_slice(&[0x47, 0x88, 0x0c, 0x1a]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    code.extend_from_slice(&[0x48, 0xff, 0xc9]);
    code.extend_from_slice(&[
        0x75,
        (copy_loop as isize - (code.len() as isize + 2)) as i8 as u8,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    let empty_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, empty_target);
}

fn emit_read_file_helper(code: &mut Vec<u8>, layout: &Layout) {
    // Win64 ABI: reserve shadow space plus locals for the handle, size, buffer, and byte count.
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);

    // CreateFileA(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL).
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x00, 0x00, 0x00, 0x80]);
    code.extend_from_slice(&[0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x84);

    // size = GetFileSize(handle, NULL), then allocate size + 1 bytes.
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x31, 0xd2]);
    emit_call_iat(code, layout, layout.get_file_size_iat);
    code.extend_from_slice(&[0x89, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x84);

    // ReadFile(handle, buffer, size, &bytes_read, NULL).
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x44, 0x8b, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x3c]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.read_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x8b, 0x4c, 0x24, 0x3c]);
    code.extend_from_slice(&[0x88, 0x0c, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x58, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let no_handle = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_near_jump(code, invalid_handle, no_handle);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, read_failed, failure);
}

fn emit_write_file_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let null_data = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    code.extend_from_slice(&[0x31, 0xc0]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0x3a, 0x00]);
    let length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, length_loop);
    let length_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x50]);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x00, 0x00, 0x00, 0x40]);
    code.extend_from_slice(&[0x45, 0x31, 0xc0]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x02, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x44, 0x8b, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x3c]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.write_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let write_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let null_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x58, 0xc3]);

    patch_near_jump(code, null_data, null_target);
    patch_near_jump(code, invalid_handle, failure);
    patch_near_jump(code, write_failed, failure);
    patch_short_jump(code, length_done, length_target);
}

fn emit_append_file_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let null_data = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    code.extend_from_slice(&[0x45, 0x31, 0xdb]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x45, 0x80, 0x3a, 0x00]);
    let length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    emit_short_jump_back(code, length_loop);
    let length_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0x5c, 0x24, 0x50]);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x04, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x45, 0x31, 0xc0]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x04, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x44, 0x8b, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x3c]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.write_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let write_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x8b, 0x44, 0x24, 0x3c]);
    code.extend_from_slice(&[0x3b, 0x44, 0x24, 0x50]);
    let short_write = emit_near_jump_placeholder(code, 0x85);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let no_handle = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_near_jump(code, null_path, no_handle);
    patch_near_jump(code, null_data, length_target);
    patch_near_jump(code, invalid_handle, no_handle);
    patch_near_jump(code, write_failed, failure);
    patch_near_jump(code, short_write, failure);
    patch_short_jump(code, length_done, length_target);
}

fn emit_touch_file_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0xba, 0x00, 0x00, 0x00, 0x40]);
    code.extend_from_slice(&[0x45, 0x31, 0xc0]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x04, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0xc1]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_near_jump(code, null_path, failure);
    patch_near_jump(code, invalid_handle, failure);
}

fn emit_remove_file_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    emit_call_iat(code, layout, layout.delete_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_file_open_helper(
    code: &mut Vec<u8>,
    layout: &Layout,
    access: u32,
    share_mode: u32,
    creation: u32,
) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0xba]);
    code.extend_from_slice(&access.to_le_bytes());
    code.extend_from_slice(&[0x41, 0xb8]);
    code.extend_from_slice(&share_mode.to_le_bytes());
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&creation.to_le_bytes());
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0, 0, 0, 0]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0, 0, 0, 0]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_near_jump(code, null_path, failure);
}

fn emit_file_write_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x49, 0x89, 0xd2]);
    code.extend_from_slice(&[0x45, 0x31, 0xdb]);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let null_data = emit_near_jump_placeholder(code, 0x84);
    let length_loop = code.len();
    code.extend_from_slice(&[0x45, 0x80, 0x3a, 0x00]);
    let length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc3]);
    emit_short_jump_back(code, length_loop);
    let length_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0x5c, 0x24, 0x50]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x48]);
    code.extend_from_slice(&[0x44, 0x8b, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x3c]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]);
    emit_call_iat(code, layout, layout.write_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let write_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x8b, 0x44, 0x24, 0x3c]);
    code.extend_from_slice(&[0x3b, 0x44, 0x24, 0x50]);
    let short_write = emit_near_jump_placeholder(code, 0x85);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_near_jump(code, null_data, length_target);
    patch_near_jump(code, write_failed, failure);
    patch_near_jump(code, short_write, failure);
    patch_short_jump(code, length_done, length_target);
}

fn emit_file_close_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
}

fn emit_file_exists_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let non_null = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let call_target = code.len();
    emit_call_iat(code, layout, layout.get_file_attributes_iat);
    code.extend_from_slice(&[0x83, 0xf8, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let missing_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, non_null, call_target);
    patch_short_jump(code, missing, missing_target);
}

fn emit_file_attribute_helper(code: &mut Vec<u8>, layout: &Layout, directory: bool) {
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    emit_call_iat(code, layout, layout.get_file_attributes_iat);
    code.extend_from_slice(&[0x83, 0xf8, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xf7, 0xc0, 0x10, 0x00, 0x00, 0x00]);
    code.extend_from_slice(if directory {
        &[0x0f, 0x95, 0xc0]
    } else {
        &[0x0f, 0x94, 0xc0]
    });
    code.extend_from_slice(&[0x0f, 0xb6, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, missing, failure);
}

fn emit_file_size_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xba, 0x00, 0x00, 0x00, 0x80]);
    code.extend_from_slice(&[0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x30, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.create_file_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let invalid_handle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x31, 0xd2]);
    emit_call_iat(code, layout, layout.get_file_size_iat);
    code.extend_from_slice(&[0x89, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    emit_call_iat(code, layout, layout.close_handle_iat);
    code.extend_from_slice(&[0x8b, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, invalid_handle, failure);
}

fn emit_file_is_empty_helper(code: &mut Vec<u8>, layout: &Layout, file_size: u32) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    emit_direct_call(code, layout.text_rva, file_size);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x94, 0xc0, 0x0f, 0xb6, 0xc0]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
}

fn emit_file_read_to_string_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let null_handle = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x31, 0xd2]);
    emit_call_iat(code, layout, layout.get_file_size_iat);
    code.extend_from_slice(&[0x89, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x83, 0xf8, 0xff]);
    let size_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x84);

    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x44, 0x8b, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0, 0, 0, 0]);
    emit_call_iat(code, layout, layout.read_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x8b, 0x4c, 0x24, 0x48]);
    code.extend_from_slice(&[0xc6, 0x04, 0x08, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x58, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
    patch_near_jump(code, null_handle, failure);
    patch_near_jump(code, size_failed, failure);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, read_failed, failure);
}

fn emit_read_file_or_helper(code: &mut Vec<u8>, layout: &Layout, read_file_rva: u32) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x20]);
    emit_direct_call(code, layout.text_rva, read_file_rva);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let has_value = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    let done = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_short_jump(code, has_value, done);
}

fn emit_read_line_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48]);

    // GetStdHandle(STD_INPUT_HANDLE), then reserve a bounded input buffer.
    code.extend_from_slice(&[0xb9, 0xf6, 0xff, 0xff, 0xff]);
    emit_call_iat(code, layout, layout.get_std_handle_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x31, 0xc9]);
    code.extend_from_slice(&[0xba, 0x00, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xb9, 0x04, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.virtual_alloc_iat);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x84);

    // ReadFile(handle, buffer, 4095, &bytes_read, NULL).
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x40]);
    code.extend_from_slice(&[0x41, 0xb8, 0xff, 0x0f, 0x00, 0x00]);
    code.extend_from_slice(&[0x4c, 0x8d, 0x4c, 0x24, 0x34]);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
    emit_call_iat(code, layout, layout.read_file_iat);
    code.extend_from_slice(&[0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x84);

    // Truncate at the first newline, otherwise terminate at bytes_read.
    code.extend_from_slice(&[0x4c, 0x8b, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x8b, 0x4c, 0x24, 0x34]);
    code.extend_from_slice(&[0x48, 0x89, 0xca]);
    code.extend_from_slice(&[0x31, 0xc0]);
    let scan = code.len();
    code.extend_from_slice(&[0x48, 0x39, 0xd0]);
    let scan_done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[0x41, 0x80, 0x3c, 0x00, 0x0a]);
    let newline = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, scan);
    let newline_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    let terminate = code.len();
    code.extend_from_slice(&[0x41, 0xc6, 0x04, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, read_failed, failure);
    patch_short_jump(code, scan_done, terminate);
    patch_short_jump(code, newline, newline_target);
}

fn build_compiled_rdata(image: &ObjectImage, needs_console_helpers: bool) -> Vec<u8> {
    let mut data = image.rodata.clone();
    if needs_console_helpers {
        data.push(b'\n');
        data.push(0);
    }
    pad_to(&mut data, FILE_ALIGNMENT as usize);
    data
}

fn emit_direct_call(code: &mut Vec<u8>, text_rva: u32, target_rva: u32) {
    let next_rva = text_rva + code.len() as u32 + 5;
    code.push(0xe8);
    code.extend_from_slice(&rel32(target_rva, next_rva).to_le_bytes());
}

fn emit_call_iat(code: &mut Vec<u8>, layout: &Layout, target_rva: u32) {
    let next_rva = layout.text_rva + code.len() as u32 + 6;
    code.extend_from_slice(&[0xff, 0x15]);
    code.extend_from_slice(&rel32(target_rva, next_rva).to_le_bytes());
}

fn emit_lea(code: &mut Vec<u8>, layout: &Layout, opcode: &[u8], target_rva: u32) {
    let next_rva = layout.text_rva + code.len() as u32 + opcode.len() as u32 + 4;
    code.extend_from_slice(opcode);
    code.extend_from_slice(&rel32(target_rva, next_rva).to_le_bytes());
}

fn rel32(target: u32, next: u32) -> i32 {
    (target as i64 - next as i64) as i32
}

fn build_rdata(message: &str) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(message.as_bytes());
    data.push(0);
    pad_to(&mut data, 8);
    data.extend_from_slice(&0_u64.to_le_bytes());
    pad_to(&mut data, FILE_ALIGNMENT as usize);
    data
}

fn build_idata(layout: &Layout) -> Vec<u8> {
    let mut data = vec![0_u8; FILE_ALIGNMENT as usize];
    let oft = 40_u32;
    let ft = oft + layout.iat_size;
    let dll = ft + layout.iat_size;
    let first_name = align_to(dll + b"KERNEL32.dll\0".len() as u32, 2);

    write_u32(&mut data, 0, layout.idata_rva + oft);
    write_u32(&mut data, 12, layout.idata_rva + dll);
    write_u32(&mut data, 16, layout.idata_rva + ft);

    let mut imports = Vec::new();
    if layout.has_console_io {
        imports.push(b"GetStdHandle\0".as_slice());
        imports.push(b"WriteFile\0".as_slice());
    }
    imports.push(b"ExitProcess\0".as_slice());
    if layout.has_virtual_alloc {
        imports.push(b"VirtualAlloc\0".as_slice());
        imports.push(b"VirtualFree\0".as_slice());
    }
    if layout.has_file_read {
        imports.push(b"CreateFileA\0".as_slice());
        imports.push(b"GetFileSize\0".as_slice());
        imports.push(b"ReadFile\0".as_slice());
        imports.push(b"CloseHandle\0".as_slice());
    } else if layout.has_file_ops {
        imports.push(b"CreateFileA\0".as_slice());
        imports.push(b"CloseHandle\0".as_slice());
    }
    if layout.has_file_ops {
        imports.push(b"DeleteFileA\0".as_slice());
    }
    if layout.has_file_metadata {
        imports.push(b"GetFileAttributesA\0".as_slice());
    }

    let mut name_offset = first_name;
    for (idx, name) in imports.iter().enumerate() {
        let name_rva = layout.idata_rva + name_offset;
        write_u64(&mut data, oft as usize + idx * 8, u64::from(name_rva));
        write_u64(&mut data, ft as usize + idx * 8, u64::from(name_rva));
        write_import_name(&mut data, name_offset as usize, name);
        name_offset = align_to(name_offset + 2 + name.len() as u32, 2);
    }

    write_bytes(&mut data, dll as usize, b"KERNEL32.dll\0");
    data
}

fn write_import_name(data: &mut [u8], offset: usize, name: &[u8]) {
    data[offset..offset + 2].copy_from_slice(&0_u16.to_le_bytes());
    write_bytes(data, offset + 2, name);
}

fn build_image(layout: &Layout, text: &[u8], rdata: &[u8], idata: &[u8]) -> Vec<u8> {
    let text_raw = layout.text_raw;
    let rdata_raw = text_raw + align_to(text.len() as u32, FILE_ALIGNMENT);
    let rdata_file_size = align_to(
        rdata.len().max(FILE_ALIGNMENT as usize) as u32,
        FILE_ALIGNMENT,
    );
    let idata_raw = rdata_raw + rdata_file_size;
    let file_size = idata_raw as usize + idata.len();
    let mut out = vec![0_u8; file_size];
    write_dos_header(&mut out);
    write_pe_headers(&mut out, layout);
    write_section_header(
        &mut out,
        0x188,
        b".text\0\0\0",
        text.len() as u32,
        layout.text_rva,
        text_raw,
        align_to(text.len() as u32, FILE_ALIGNMENT),
        0x60000020,
    );
    write_section_header(
        &mut out,
        0x1b0,
        b".rdata\0\0",
        rdata.len() as u32,
        layout.rdata_rva,
        rdata_raw,
        rdata_file_size,
        0xc0000040,
    );
    write_section_header(
        &mut out,
        0x1d8,
        b".idata\0\0",
        idata.len() as u32,
        layout.idata_rva,
        idata_raw,
        align_to(idata.len() as u32, FILE_ALIGNMENT),
        0xc0000040,
    );
    write_bytes(&mut out, text_raw as usize, text);
    write_bytes(&mut out, rdata_raw as usize, rdata);
    write_bytes(&mut out, idata_raw as usize, idata);
    out
}

fn write_dos_header(out: &mut [u8]) {
    out[0..2].copy_from_slice(b"MZ");
    write_u32(out, 0x3c, 0x80);
}

fn write_pe_headers(out: &mut [u8], layout: &Layout) {
    let pe = 0x80;
    out[pe..pe + 4].copy_from_slice(b"PE\0\0");
    write_u16(out, pe + 4, 0x8664);
    write_u16(out, pe + 6, 3);
    write_u16(out, pe + 20, 0xf0);
    write_u16(out, pe + 22, 0x22);

    let opt = pe + 24;
    write_u16(out, opt, 0x20b);
    out[opt + 2] = 1;
    write_u32(out, opt + 4, FILE_ALIGNMENT);
    write_u32(out, opt + 8, FILE_ALIGNMENT * 2);
    write_u32(out, opt + 16, layout.text_rva);
    write_u32(out, opt + 20, layout.text_rva);
    write_u64(out, opt + 24, IMAGE_BASE);
    write_u32(out, opt + 32, SECTION_ALIGNMENT);
    write_u32(out, opt + 36, FILE_ALIGNMENT);
    write_u16(out, opt + 40, 6);
    write_u16(out, opt + 48, 6);
    write_u32(out, opt + 56, 0x4000);
    write_u32(out, opt + 60, 0x200);
    write_u16(out, opt + 68, 3);
    write_u64(out, opt + 72, 0x100000);
    write_u64(out, opt + 80, 0x1000);
    write_u64(out, opt + 88, 0x100000);
    write_u64(out, opt + 96, 0x1000);
    write_u32(out, opt + 108, 16);
    write_u32(out, opt + 120, layout.import_descriptor_rva);
    write_u32(out, opt + 124, layout.import_descriptor_size);
    write_u32(out, opt + 208, layout.get_std_handle_iat);
    write_u32(out, opt + 212, layout.iat_size);
}

fn write_section_header(
    out: &mut [u8],
    offset: usize,
    name: &[u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    raw_pointer: u32,
    raw_size: u32,
    characteristics: u32,
) {
    out[offset..offset + 8].copy_from_slice(name);
    write_u32(out, offset + 8, virtual_size);
    write_u32(out, offset + 12, virtual_address);
    write_u32(out, offset + 16, raw_size);
    write_u32(out, offset + 20, raw_pointer);
    write_u32(out, offset + 36, characteristics);
}

fn write_bytes(out: &mut [u8], offset: usize, bytes: &[u8]) {
    out[offset..offset + bytes.len()].copy_from_slice(bytes);
}

fn write_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut [u8], offset: usize, value: u64) {
    out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
    while bytes.len() % alignment != 0 {
        bytes.push(0);
    }
}
