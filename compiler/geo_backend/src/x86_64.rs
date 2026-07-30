use crate::ir::{CmpOp, Instruction, IrFunction, IrProgram, ValueId};
use crate::target::{Abi, Target};
use std::collections::{BTreeSet, HashMap};

pub fn emit_nasm(program: &IrProgram) -> String {
    emit_nasm_for_target(program, &Target::linux_x86_64())
}

pub fn emit_nasm_for_target(program: &IrProgram, target: &Target) -> String {
    emit_nasm_for_target_with_runtime_entry(program, target, false)
}

pub fn emit_nasm_for_target_with_runtime_entry(
    program: &IrProgram,
    target: &Target,
    runtime_entry: bool,
) -> String {
    let mut out = String::new();
    if runtime_entry {
        out.push_str("global geo_main\n");
    } else {
        out.push_str("global main\n");
    }
    emit_external_declarations(program, &mut out);
    emit_data_section(program, &mut out);
    out.push_str("section .text\n\n");

    for function in &program.functions {
        emit_function(function, target, runtime_entry, &mut out);
    }

    out
}

fn emit_external_declarations(program: &IrProgram, out: &mut String) {
    let defined: BTreeSet<&str> = program
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    let mut externs = BTreeSet::new();

    for function in &program.functions {
        for instruction in &function.instructions {
            match instruction {
                Instruction::Call { function, .. } if !defined.contains(function.as_str()) => {
                    externs.insert(function.as_str());
                }
                Instruction::CallAggregate { function, .. }
                    if !defined.contains(function.as_str()) =>
                {
                    externs.insert(function.as_str());
                }
                Instruction::BoundsCheck { .. } => {
                    externs.insert("__geo_bounds_check");
                }
                _ => {}
            }
        }
    }

    for symbol in externs {
        out.push_str(&format!("extern {symbol}\n"));
    }
    if !out.ends_with("global main\n") && !out.ends_with("global geo_main\n") {
        out.push('\n');
    }
}

struct FunctionLayout {
    value_slots: HashMap<ValueId, i32>,
    local_slots: HashMap<String, i32>,
    aggregate_buffers: HashMap<usize, i32>,
    frame_size: i32,
}

fn emit_function(function: &IrFunction, target: &Target, runtime_entry: bool, out: &mut String) {
    let layout = build_layout(function);
    out.push_str(&format!(
        "{}:\n",
        emitted_function_name(&function.name, runtime_entry)
    ));
    out.push_str("    push rbp\n");
    out.push_str("    mov rbp, rsp\n");
    if layout.frame_size > 0 {
        out.push_str(&format!("    sub rsp, {}\n", layout.frame_size));
    }

    let arg_regs = arg_registers(target.abi);
    for (idx, param) in function.params.iter().enumerate() {
        if let Some(slot) = layout.local_slots.get(param) {
            if idx < arg_regs.len() {
                out.push_str(&format!("    mov [rbp - {slot}], {}\n", arg_regs[idx]));
            } else {
                let source = stack_param_offset(target.abi, idx);
                out.push_str(&format!("    mov rax, [rbp + {source}]\n"));
                out.push_str(&format!("    mov [rbp - {slot}], rax\n"));
            }
        }
    }

    for instruction in &function.instructions {
        match instruction {
            Instruction::Const { dst, value } => {
                let dst = slot(&layout, *dst);
                out.push_str(&format!("    mov qword [rbp - {dst}], {value}\n"));
            }
            Instruction::StringConst { dst, label, .. } => {
                let dst = slot(&layout, *dst);
                out.push_str(&format!("    lea rax, [rel {label}]\n"));
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::Load { dst, local } => {
                let dst = slot(&layout, *dst);
                let local = local_slot(&layout, local);
                out.push_str(&format!("    mov rax, [rbp - {local}]\n"));
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::AddressOf { dst, local } => {
                let dst = slot(&layout, *dst);
                let local = local_slot(&layout, local);
                out.push_str(&format!("    lea rax, [rbp - {local}]\n"));
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::Deref {
                dst,
                pointer,
                width,
            } => {
                let dst = slot(&layout, *dst);
                let pointer = slot(&layout, *pointer);
                out.push_str(&format!("    mov rax, [rbp - {pointer}]\n"));
                match width {
                    1 => out.push_str("    movzx eax, byte [rax]\n"),
                    2 => out.push_str("    movzx eax, word [rax]\n"),
                    4 => out.push_str("    mov eax, [rax]\n"),
                    _ => out.push_str("    mov rax, [rax]\n"),
                }
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::BitNot { dst, value } => {
                let dst = slot(&layout, *dst);
                let value = slot(&layout, *value);
                out.push_str(&format!("    mov rax, [rbp - {value}]\n"));
                out.push_str("    not rax\n");
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::BoundsCheck { index, len } => {
                let index = slot(&layout, *index);
                if target.abi == Abi::WindowsX64 {
                    out.push_str("    sub rsp, 32\n");
                }
                out.push_str(&format!("    mov {}, [rbp - {index}]\n", arg_regs[0]));
                out.push_str(&format!("    mov {}, {len}\n", arg_regs[1]));
                out.push_str("    call __geo_bounds_check\n");
                if target.abi == Abi::WindowsX64 {
                    out.push_str("    add rsp, 32\n");
                }
            }
            Instruction::Store { local, value } => {
                let value = slot(&layout, *value);
                let local = local_slot(&layout, local);
                out.push_str(&format!("    mov rax, [rbp - {value}]\n"));
                out.push_str(&format!("    mov [rbp - {local}], rax\n"));
            }
            Instruction::StoreDeref {
                pointer,
                value,
                width,
            } => {
                let pointer = slot(&layout, *pointer);
                let value = slot(&layout, *value);
                out.push_str(&format!("    mov rax, [rbp - {pointer}]\n"));
                out.push_str(&format!("    mov r10, [rbp - {value}]\n"));
                match width {
                    1 => out.push_str("    mov byte [rax], r10b\n"),
                    2 => out.push_str("    mov word [rax], r10w\n"),
                    4 => out.push_str("    mov dword [rax], r10d\n"),
                    _ => out.push_str("    mov [rax], r10\n"),
                }
            }
            Instruction::And { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "and");
            }
            Instruction::Or { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "or");
            }
            Instruction::BitAnd { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "and");
            }
            Instruction::BitOr { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "or");
            }
            Instruction::BitXor { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "xor");
            }
            Instruction::ShiftLeft { dst, left, right } => {
                emit_shift(out, &layout, *dst, *left, *right, "shl");
            }
            Instruction::ShiftRight { dst, left, right } => {
                emit_shift(out, &layout, *dst, *left, *right, "sar");
            }
            Instruction::Add { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "add");
            }
            Instruction::Sub { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "sub");
            }
            Instruction::Mul { dst, left, right } => {
                emit_binary(out, &layout, *dst, *left, *right, "imul");
            }
            Instruction::Div { dst, left, right } => {
                let dst = slot(&layout, *dst);
                let left = slot(&layout, *left);
                let right = slot(&layout, *right);
                out.push_str(&format!("    mov rax, [rbp - {left}]\n"));
                out.push_str("    cqo\n");
                out.push_str(&format!("    mov r10, [rbp - {right}]\n"));
                out.push_str("    idiv r10\n");
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::Rem { dst, left, right } => {
                let dst = slot(&layout, *dst);
                let left = slot(&layout, *left);
                let right = slot(&layout, *right);
                out.push_str(&format!("    mov rax, [rbp - {left}]\n"));
                out.push_str("    cqo\n");
                out.push_str(&format!("    mov r10, [rbp - {right}]\n"));
                out.push_str("    idiv r10\n");
                out.push_str(&format!("    mov [rbp - {dst}], rdx\n"));
            }
            Instruction::Cmp {
                dst,
                op,
                left,
                right,
            } => {
                let dst = slot(&layout, *dst);
                let left = slot(&layout, *left);
                let right = slot(&layout, *right);
                out.push_str(&format!("    mov rax, [rbp - {left}]\n"));
                out.push_str(&format!("    cmp rax, [rbp - {right}]\n"));
                out.push_str(&format!("    {} al\n", setcc(*op)));
                out.push_str("    movzx rax, al\n");
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::Jump { label } => {
                out.push_str(&format!("    jmp {label}\n"));
            }
            Instruction::JumpIfZero { value, label } => {
                let value = slot(&layout, *value);
                out.push_str(&format!("    mov rax, [rbp - {value}]\n"));
                out.push_str("    cmp rax, 0\n");
                out.push_str(&format!("    je {label}\n"));
            }
            Instruction::Label { name } => {
                out.push_str(&format!("{name}:\n"));
            }
            Instruction::Call {
                dst,
                function,
                args,
            } => {
                let stack_arg_count = args.len().saturating_sub(arg_regs.len());
                for arg in args.iter().skip(arg_regs.len()).rev() {
                    let arg = slot(&layout, *arg);
                    out.push_str(&format!("    push qword [rbp - {arg}]\n"));
                }
                if target.abi == Abi::WindowsX64 {
                    out.push_str("    sub rsp, 32\n");
                }
                for (idx, arg) in args.iter().take(arg_regs.len()).enumerate() {
                    let arg = slot(&layout, *arg);
                    out.push_str(&format!("    mov {}, [rbp - {arg}]\n", arg_regs[idx]));
                }
                out.push_str(&format!("    call {function}\n"));
                let cleanup =
                    stack_arg_count * 8 + if target.abi == Abi::WindowsX64 { 32 } else { 0 };
                if cleanup > 0 {
                    out.push_str(&format!("    add rsp, {cleanup}\n"));
                }
                let dst = slot(&layout, *dst);
                out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
            }
            Instruction::CallAggregate {
                dst,
                function,
                args,
                buffer,
            } => {
                let total_args = args.len() + 1;
                let stack_arg_count = total_args.saturating_sub(arg_regs.len());
                for index in (arg_regs.len()..total_args).rev() {
                    let arg = slot(&layout, args[index - 1]);
                    out.push_str(&format!("    push qword [rbp - {arg}]\n"));
                }
                if target.abi == Abi::WindowsX64 {
                    out.push_str("    sub rsp, 32\n");
                }
                let buffer = layout.aggregate_buffer(*buffer);
                out.push_str(&format!("    lea {}, [rbp - {buffer}]\n", arg_regs[0]));
                for (index, arg) in args
                    .iter()
                    .take(arg_regs.len().saturating_sub(1))
                    .enumerate()
                {
                    let arg = slot(&layout, *arg);
                    out.push_str(&format!("    mov {}, [rbp - {arg}]\n", arg_regs[index + 1]));
                }
                out.push_str(&format!("    call {function}\n"));
                let cleanup =
                    stack_arg_count * 8 + if target.abi == Abi::WindowsX64 { 32 } else { 0 };
                if cleanup > 0 {
                    out.push_str(&format!("    add rsp, {cleanup}\n"));
                }
                for (index, dst) in dst.iter().enumerate() {
                    let dst = slot(&layout, *dst);
                    let offset = buffer + index as i32 * 8;
                    out.push_str(&format!("    mov rax, [rbp - {offset}]\n"));
                    out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
                }
            }
            Instruction::ReturnAggregate { values } => {
                let pointer = local_slot(&layout, "__geo_return_ptr");
                out.push_str(&format!("    mov rax, [rbp - {pointer}]\n"));
                for (index, value) in values.iter().enumerate() {
                    let value = slot(&layout, *value);
                    let offset = index as i32 * -8;
                    out.push_str(&format!("    mov r10, [rbp - {value}]\n"));
                    if offset == 0 {
                        out.push_str("    mov [rax], r10\n");
                    } else {
                        out.push_str(&format!("    mov [rax - {}], r10\n", -offset));
                    }
                }
                out.push_str("    mov rsp, rbp\n    pop rbp\n    ret\n");
            }
            Instruction::Return { value } => {
                let value = slot(&layout, *value);
                out.push_str(&format!("    mov rax, [rbp - {value}]\n"));
                out.push_str("    mov rsp, rbp\n");
                out.push_str("    pop rbp\n");
                out.push_str("    ret\n");
            }
        }
    }

    out.push('\n');
}

fn emitted_function_name(name: &str, runtime_entry: bool) -> &str {
    if runtime_entry && name == "main" {
        "geo_main"
    } else {
        name
    }
}

fn arg_registers(abi: Abi) -> &'static [&'static str] {
    match abi {
        Abi::SystemV => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
        Abi::WindowsX64 => &["rcx", "rdx", "r8", "r9"],
    }
}

fn stack_param_offset(abi: Abi, param_index: usize) -> usize {
    let register_count = arg_registers(abi).len();
    let stack_index = param_index - register_count;
    match abi {
        Abi::SystemV => 16 + stack_index * 8,
        Abi::WindowsX64 => 48 + stack_index * 8,
    }
}

fn build_layout(function: &IrFunction) -> FunctionLayout {
    let mut next_offset = 8;
    let mut value_slots = HashMap::new();
    let mut local_slots = HashMap::new();

    for param in &function.params {
        local_slots.entry(param.clone()).or_insert_with(|| {
            let offset = next_offset;
            next_offset += 8;
            offset
        });
    }

    for instruction in &function.instructions {
        for value in defined_values(instruction) {
            value_slots.entry(value).or_insert_with(|| {
                let offset = next_offset;
                next_offset += 8;
                offset
            });
        }
        if let Instruction::Store { local, .. } = instruction {
            local_slots.entry(local.clone()).or_insert_with(|| {
                let offset = next_offset;
                next_offset += 8;
                offset
            });
        }
    }

    let mut aggregate_buffers = HashMap::new();
    for instruction in &function.instructions {
        if let Instruction::CallAggregate { dst, buffer, .. } = instruction {
            aggregate_buffers.entry(*buffer).or_insert_with(|| {
                let offset = next_offset;
                next_offset += dst.len() as i32 * 8;
                offset
            });
        }
    }

    let used = next_offset - 8;
    let frame_size = if used == 0 {
        0
    } else {
        ((used + 15) / 16) * 16
    };

    FunctionLayout {
        value_slots,
        local_slots,
        aggregate_buffers,
        frame_size,
    }
}

impl FunctionLayout {
    fn aggregate_buffer(&self, buffer: usize) -> i32 {
        *self
            .aggregate_buffers
            .get(&buffer)
            .expect("aggregate return buffer")
    }
}

fn defined_values(instruction: &Instruction) -> Vec<ValueId> {
    match instruction {
        Instruction::Const { dst, .. }
        | Instruction::StringConst { dst, .. }
        | Instruction::And { dst, .. }
        | Instruction::Or { dst, .. }
        | Instruction::BitAnd { dst, .. }
        | Instruction::BitOr { dst, .. }
        | Instruction::BitXor { dst, .. }
        | Instruction::ShiftLeft { dst, .. }
        | Instruction::ShiftRight { dst, .. }
        | Instruction::Add { dst, .. }
        | Instruction::Sub { dst, .. }
        | Instruction::Mul { dst, .. }
        | Instruction::Div { dst, .. }
        | Instruction::Rem { dst, .. }
        | Instruction::Load { dst, .. }
        | Instruction::AddressOf { dst, .. }
        | Instruction::Deref { dst, .. }
        | Instruction::BitNot { dst, .. }
        | Instruction::Cmp { dst, .. }
        | Instruction::Call { dst, .. } => vec![*dst],
        Instruction::CallAggregate { dst, .. } => dst.clone(),
        Instruction::Store { .. }
        | Instruction::StoreDeref { .. }
        | Instruction::BoundsCheck { .. }
        | Instruction::Jump { .. }
        | Instruction::JumpIfZero { .. }
        | Instruction::Label { .. }
        | Instruction::ReturnAggregate { .. }
        | Instruction::Return { .. } => Vec::new(),
    }
}

fn emit_data_section(program: &IrProgram, out: &mut String) {
    let string_consts: Vec<(&String, &String)> = program
        .functions
        .iter()
        .flat_map(|function| function.instructions.iter())
        .filter_map(|instruction| {
            if let Instruction::StringConst { label, value, .. } = instruction {
                Some((label, value))
            } else {
                None
            }
        })
        .collect();

    if string_consts.is_empty() {
        return;
    }

    out.push_str("section .data\n");
    for (label, value) in string_consts {
        out.push_str(&format!("{label}: db {}\n", nasm_bytes(value)));
    }
    out.push('\n');
}

fn nasm_bytes(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(u8::to_string)
        .chain(std::iter::once("0".to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_binary(
    out: &mut String,
    layout: &FunctionLayout,
    dst: ValueId,
    left: ValueId,
    right: ValueId,
    op: &str,
) {
    let dst = slot(layout, dst);
    let left = slot(layout, left);
    let right = slot(layout, right);
    out.push_str(&format!("    mov rax, [rbp - {left}]\n"));
    out.push_str(&format!("    mov r10, [rbp - {right}]\n"));
    out.push_str(&format!("    {op} rax, r10\n"));
    out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
}

fn emit_shift(
    out: &mut String,
    layout: &FunctionLayout,
    dst: ValueId,
    left: ValueId,
    right: ValueId,
    op: &str,
) {
    let dst = slot(layout, dst);
    let left = slot(layout, left);
    let right = slot(layout, right);
    out.push_str(&format!("    mov rax, [rbp - {left}]\n"));
    out.push_str(&format!("    mov rcx, [rbp - {right}]\n"));
    out.push_str(&format!("    {op} rax, cl\n"));
    out.push_str(&format!("    mov [rbp - {dst}], rax\n"));
}

fn slot(layout: &FunctionLayout, value: ValueId) -> i32 {
    *layout
        .value_slots
        .get(&value)
        .expect("value should have stack slot")
}

fn local_slot(layout: &FunctionLayout, local: &str) -> i32 {
    *layout
        .local_slots
        .get(local)
        .expect("local should have stack slot")
}

fn setcc(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Equal => "sete",
        CmpOp::NotEqual => "setne",
        CmpOp::Less => "setl",
        CmpOp::LessEqual => "setle",
        CmpOp::Greater => "setg",
        CmpOp::GreaterEqual => "setge",
    }
}
