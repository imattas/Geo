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
    let layout = Layout::new("", false);
    let text = build_compiled_text(&layout, &image)?;
    let rdata = build_compiled_rdata(&image);
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
    let bounds_check_rva = if image
        .relocations
        .iter()
        .any(|relocation| relocation.symbol == "__geo_bounds_check")
    {
        Some(layout.text_rva + align_to(function_base + image.text.len() as u32, 16))
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
    if let Some(bounds_check_rva) = bounds_check_rva {
        while layout.text_rva + (code.len() as u32) < bounds_check_rva {
            code.push(0xcc);
        }
        emit_bounds_check_helper(&mut code, layout);
    }
    patch_compiled_relocations(&mut code, layout, image, function_base, bounds_check_rva)?;
    pad_to(&mut code, FILE_ALIGNMENT as usize);
    Some(code)
}

fn patch_compiled_relocations(
    code: &mut [u8],
    layout: &Layout,
    image: &ObjectImage,
    function_base: u32,
    bounds_check_rva: Option<u32>,
) -> Option<()> {
    for relocation in &image.relocations {
        let relocation_offset = function_base + relocation.offset as u32;
        let target_rva = compiled_symbol_rva(
            layout,
            image,
            function_base,
            bounds_check_rva,
            &relocation.symbol,
        )?;
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
    bounds_check_rva: Option<u32>,
    symbol: &str,
) -> Option<u32> {
    if symbol == "__geo_bounds_check" {
        return bounds_check_rva;
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

fn emit_bounds_check_helper(code: &mut Vec<u8>, layout: &Layout) {
    code.extend_from_slice(&[0x48, 0x39, 0xd1]);
    code.extend_from_slice(&[0x73, 0x01]);
    code.push(0xc3);
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.push(0xb9);
    code.extend_from_slice(&1_u32.to_le_bytes());
    emit_call_iat(code, layout, layout.exit_process_iat);
}

fn build_compiled_rdata(image: &ObjectImage) -> Vec<u8> {
    let mut data = image.rodata.clone();
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
