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
    let layout = Layout::new(plan.output.as_deref().unwrap_or(""), plan.output.is_some());
    let text = build_text(&layout, &plan);
    let rdata = build_rdata(plan.output.as_deref().unwrap_or(""));
    let idata = build_idata(&layout);
    Some(build_image(&layout, &text, &rdata, &idata))
}

fn emit_compiled_pe64_console(program: &IrProgram) -> Option<Vec<u8>> {
    let image = crate::object::build_win64_code_image(program)?;
    let needs_console_helpers = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "print" || relocation.symbol == "println");
    let needs_string_concat = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_concat");
    let layout = Layout::new("", needs_console_helpers);
    let text = build_compiled_text(&layout, &image)?;
    let rdata = build_compiled_rdata(&image, needs_console_helpers, needs_string_concat);
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
                Instruction::Deref { dst, pointer } => {
                    let PeValue::LocalRef(local) = values.get(pointer)? else {
                        return None;
                    };
                    values.insert(*dst, locals.get(local)?.clone());
                }
                Instruction::StoreDeref { pointer, value } => {
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
    rdata_raw: u32,
    idata_raw: u32,
    message_rva: u32,
    written_rva: u32,
    import_descriptor_rva: u32,
    import_descriptor_size: u32,
    get_std_handle_iat: u32,
    write_file_iat: u32,
    exit_process_iat: u32,
    has_console_io: bool,
}

impl Layout {
    fn new(message: &str, has_console_io: bool) -> Self {
        let headers_raw = 0x200;
        let text_rva = 0x1000;
        let rdata_rva = 0x2000;
        let idata_rva = 0x3000;
        let text_raw = headers_raw;
        let rdata_raw = text_raw + FILE_ALIGNMENT;
        let idata_raw = rdata_raw + FILE_ALIGNMENT;
        let message_rva = rdata_rva;
        let written_rva = align_to(message_rva + message.len() as u32 + 1, 8);
        let import_descriptor_rva = idata_rva;
        let oft_rva = import_descriptor_rva + 40;
        let ft_rva = oft_rva + 32;

        Self {
            text_rva,
            rdata_rva,
            idata_rva,
            text_raw,
            rdata_raw,
            idata_raw,
            message_rva,
            written_rva,
            import_descriptor_rva,
            import_descriptor_size: FILE_ALIGNMENT,
            get_std_handle_iat: ft_rva,
            write_file_iat: ft_rva + 8,
            exit_process_iat: if has_console_io { ft_rva + 16 } else { ft_rva },
            has_console_io,
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
    let needs_string_concat = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_concat");
    let needs_string_len = image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "string_len");
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
    let newline_rva = if needs_println {
        Some(layout.rdata_rva + image.rodata.len() as u32)
    } else {
        None
    };
    let concat_buffer_rva = if needs_string_concat {
        let after_newline = image.rodata.len() as u32 + u32::from(needs_println) * 2;
        Some(layout.rdata_rva + align_to(after_newline, 8))
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
    {
        let helper_start_rva = layout.text_rva + align_to(code.len() as u32, 16);
        while layout.text_rva + (code.len() as u32) < helper_start_rva {
            code.push(0xcc);
        }
    }
    if needs_string_concat {
        helpers.string_concat = Some(layout.text_rva + code.len() as u32);
        emit_string_concat_helper(&mut code, layout, concat_buffer_rva?);
    }
    if needs_string_len {
        helpers.string_len = Some(layout.text_rva + code.len() as u32);
        emit_string_len_helper(&mut code);
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
    string_concat: Option<u32>,
    string_len: Option<u32>,
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
}

fn emit_string_concat_helper(code: &mut Vec<u8>, layout: &Layout, buffer_rva: u32) {
    emit_lea(code, layout, &[0x48, 0x8d, 0x05], buffer_rva);
    code.extend_from_slice(&[0x49, 0x89, 0xc0]);
    code.extend_from_slice(&[0x44, 0x8a, 0x09]);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x00]);
    code.extend_from_slice(&[0x74, 0x0b]);
    code.extend_from_slice(&[0x45, 0x88, 0x08]);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xec]);
    code.extend_from_slice(&[0x44, 0x8a, 0x0a]);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x00]);
    code.extend_from_slice(&[0x74, 0x0b]);
    code.extend_from_slice(&[0x45, 0x88, 0x08]);
    code.extend_from_slice(&[0x48, 0xff, 0xc2]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xec]);
    code.extend_from_slice(&[0x41, 0xc6, 0x00, 0x00]);
    code.push(0xc3);
}

fn emit_string_len_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc0]);
    code.extend_from_slice(&[0x80, 0x3c, 0x01, 0x00]);
    code.extend_from_slice(&[0x74, 0x05]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf5]);
    code.push(0xc3);
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
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x92, 0xc0]);
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
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x96, 0xc0]);
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
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x97, 0xc0]);
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
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0x93, 0xc0]);
    code.push(0xc3);

    patch_short_jump(code, difference, difference_target);
    patch_short_jump(code, matched, true_target);
}

fn emit_string_is_empty_helper(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x80, 0x39, 0x00]);
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
    code.extend_from_slice(&[0x4d, 0x31, 0xc0]);
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x01, 0x00]);
    code.extend_from_slice(&[0x74, 0x05]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf3]);
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

fn build_compiled_rdata(
    image: &ObjectImage,
    needs_console_helpers: bool,
    needs_string_concat: bool,
) -> Vec<u8> {
    let mut data = image.rodata.clone();
    if needs_console_helpers {
        data.push(b'\n');
        data.push(0);
    }
    if needs_string_concat {
        pad_to(&mut data, 8);
        data.extend(std::iter::repeat_n(0, 4096));
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
    let ft = oft + 32;
    let dll = ft + 32;
    let first_name = align_to(dll + b"KERNEL32.dll\0".len() as u32, 2);

    write_u32(&mut data, 0, layout.idata_rva + oft);
    write_u32(&mut data, 12, layout.idata_rva + dll);
    write_u32(&mut data, 16, layout.idata_rva + ft);

    if layout.has_console_io {
        let get_std = first_name;
        let write_file = align_to(get_std + 2 + b"GetStdHandle\0".len() as u32, 2);
        let exit_process = align_to(write_file + 2 + b"WriteFile\0".len() as u32, 2);
        for (idx, name_rva) in [
            layout.idata_rva + get_std,
            layout.idata_rva + write_file,
            layout.idata_rva + exit_process,
        ]
        .iter()
        .enumerate()
        {
            write_u64(&mut data, oft as usize + idx * 8, u64::from(*name_rva));
            write_u64(&mut data, ft as usize + idx * 8, u64::from(*name_rva));
        }
        write_import_name(&mut data, get_std as usize, b"GetStdHandle\0");
        write_import_name(&mut data, write_file as usize, b"WriteFile\0");
        write_import_name(&mut data, exit_process as usize, b"ExitProcess\0");
    } else {
        let exit_process = first_name;
        write_u64(
            &mut data,
            oft as usize,
            u64::from(layout.idata_rva + exit_process),
        );
        write_u64(
            &mut data,
            ft as usize,
            u64::from(layout.idata_rva + exit_process),
        );
        write_import_name(&mut data, exit_process as usize, b"ExitProcess\0");
    }

    write_bytes(&mut data, dll as usize, b"KERNEL32.dll\0");
    data
}

fn write_import_name(data: &mut [u8], offset: usize, name: &[u8]) {
    data[offset..offset + 2].copy_from_slice(&0_u16.to_le_bytes());
    write_bytes(data, offset + 2, name);
}

fn build_image(layout: &Layout, text: &[u8], rdata: &[u8], idata: &[u8]) -> Vec<u8> {
    let mut out = vec![0_u8; 0x200];
    write_dos_header(&mut out);
    write_pe_headers(&mut out, layout);
    write_section_header(
        &mut out,
        0x188,
        b".text\0\0\0",
        text.len() as u32,
        layout.text_rva,
        layout.text_raw,
        0x60000020,
    );
    write_section_header(
        &mut out,
        0x1b0,
        b".rdata\0\0",
        rdata.len() as u32,
        layout.rdata_rva,
        layout.rdata_raw,
        0xc0000040,
    );
    write_section_header(
        &mut out,
        0x1d8,
        b".idata\0\0",
        idata.len() as u32,
        layout.idata_rva,
        layout.idata_raw,
        0xc0000040,
    );
    out.extend_from_slice(text);
    out.extend_from_slice(rdata);
    out.extend_from_slice(idata);
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
    write_u32(out, opt + 212, 32);
}

fn write_section_header(
    out: &mut [u8],
    offset: usize,
    name: &[u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    raw_pointer: u32,
    characteristics: u32,
) {
    out[offset..offset + 8].copy_from_slice(name);
    write_u32(out, offset + 8, virtual_size);
    write_u32(out, offset + 12, virtual_address);
    write_u32(out, offset + 16, FILE_ALIGNMENT);
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
