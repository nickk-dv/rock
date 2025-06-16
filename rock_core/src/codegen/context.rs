use super::llvm;
use crate::error::ErrorBuffer;
use crate::hir_lower::layout;
use crate::hir_lower::types;
use crate::intern::{LitID, NameID};
use crate::session::config::TargetTriple;
use crate::session::{ModuleID, Session};
use crate::support::{Arena, AsStr, TempBuffer, TempOffset};
use crate::{ast, hir};
use std::collections::HashMap;

pub struct Codegen<'c, 's, 'sref> {
    pub proc: ProcCodegen<'c>,
    pub context: llvm::IRContext,
    pub target: llvm::IRTarget,
    pub module: llvm::IRModule,
    pub build: llvm::IRBuilder,
    pub procs: Vec<(llvm::ValueFn, llvm::TypeFn)>,
    pub procs_poly: HashMap<hir::ProcKey<'c>, (llvm::ValueFn, llvm::TypeFn)>,
    pub enums: Vec<llvm::TypeStruct>,
    pub enums_poly: HashMap<hir::EnumKey<'c>, llvm::TypeStruct>,
    pub structs: Vec<llvm::TypeStruct>,
    pub structs_poly: HashMap<hir::StructKey<'c>, llvm::TypeStruct>,
    pub globals: Vec<llvm::ValueGlobal>,
    pub strings: HashMap<LitID, llvm::ValueGlobal>,
    pub hir: hir::Hir<'c>,
    pub session: &'sref mut Session<'s>,
    pub namebuf: String,
    pub cache: CodegenCache<'c>,
    pub info: CodegenTypeInfo<'c>,
    pub proc_queue: Vec<hir::ProcKey<'c>>,
    pub emit: ErrorBuffer,
}

pub struct ProcCodegen<'c> {
    pub proc_id: hir::ProcID,
    pub fn_val: llvm::ValueFn,
    pub param_ptrs: Vec<llvm::ValuePtr>,
    pub variable_ptrs: Vec<llvm::ValuePtr>,
    pub poly_types: &'c [hir::Type<'c>],
    tail_values: Vec<Option<TailValue>>,
    block_stack: Vec<BlockInfo>,
    next_loop_info: Option<LoopInfo>,
}

struct BlockInfo {
    loop_info: Option<LoopInfo>,
}

#[derive(Copy, Clone)]
pub struct LoopInfo {
    pub break_bb: llvm::BasicBlock,
    pub continue_bb: llvm::BasicBlock,
}

#[derive(Copy, Clone)]
pub enum Expect {
    Value(Option<TailValueID>),
    Pointer(bool),
    Store(llvm::ValuePtr),
}

crate::define_id!(pub TailValueID);
#[derive(Copy, Clone)]
pub struct TailValue {
    pub value_ptr: llvm::ValuePtr,
    pub value_ty: llvm::Type,
}

pub struct CodegenCache<'c> {
    int_1: llvm::Type,
    int_8: llvm::Type,
    int_16: llvm::Type,
    int_32: llvm::Type,
    int_64: llvm::Type,
    float_32: llvm::Type,
    float_64: llvm::Type,
    void_type: llvm::Type,
    ptr_type: llvm::Type,
    ptr_sized_int: llvm::Type,
    slice_type: llvm::TypeStruct,
    void_val_type: llvm::TypeStruct,
    pub zero_i8: llvm::Value,
    pub sret: llvm::Attribute,
    pub noreturn: llvm::Attribute,
    pub inlinehint: llvm::Attribute,
    pub u8s: TempBuffer<u8>,
    pub u64s: TempBuffer<u64>,
    pub types: TempBuffer<llvm::Type>,
    pub values: TempBuffer<llvm::Value>,
    pub const_values: TempBuffer<hir::ConstValue<'c>>,
    pub cases: TempBuffer<(llvm::Value, llvm::BasicBlock)>,
    pub hir_types: TempBuffer<hir::Type<'c>>,
    pub hir_proc_ty_params: TempBuffer<hir::ProcTypeParam<'c>>,
}

pub struct CodegenTypeInfo<'c> {
    pub var_args: Vec<(llvm::Value, &'c [hir::Type<'c>])>,
    pub type_ids: HashMap<hir::Type<'c>, u64>,
    pub types: Vec<hir::ConstValue<'c>>,
    pub enums: Vec<hir::ConstValue<'c>>,
    pub structs: Vec<hir::ConstValue<'c>>,
    pub references: Vec<hir::ConstValue<'c>>,
    pub procedures: Vec<hir::ConstValue<'c>>,
    pub slices: Vec<hir::ConstValue<'c>>,
    pub arrays: Vec<hir::ConstValue<'c>>,
    pub variants: Vec<hir::ConstValue<'c>>,
    pub variant_fields: Vec<hir::ConstValue<'c>>,
    pub fields: Vec<hir::ConstValue<'c>>,
    pub param_types: Vec<hir::ConstValue<'c>>,
    pub types_id: hir::GlobalID,
    pub enums_id: hir::GlobalID,
    pub structs_id: hir::GlobalID,
    pub references_id: hir::GlobalID,
    pub procedures_id: hir::GlobalID,
    pub slices_id: hir::GlobalID,
    pub arrays_id: hir::GlobalID,
    pub variants_id: hir::GlobalID,
    pub variant_fields_id: hir::GlobalID,
    pub fields_id: hir::GlobalID,
    pub param_types_id: hir::GlobalID,
}

impl<'c, 's, 'sref> Codegen<'c, 's, 'sref> {
    pub fn new(
        hir: hir::Hir<'c>,
        triple: TargetTriple,
        session: &'sref mut Session<'s>,
    ) -> Codegen<'c, 's, 'sref> {
        let mut context = llvm::IRContext::new();
        let target = llvm::IRTarget::new(triple, session.config.build);
        let module = llvm::IRModule::new(&context, &target, "rock_module");
        let cache = CodegenCache::new(&mut context, &target);
        let info = CodegenTypeInfo::new();
        let build = llvm::IRBuilder::new(&context, cache.void_val_type);

        Codegen {
            proc: ProcCodegen::new(),
            context,
            target,
            module,
            build,
            procs: Vec::with_capacity(hir.procs.len()),
            procs_poly: HashMap::with_capacity((hir.procs.len() / 4).next_power_of_two()),
            enums: Vec::with_capacity(hir.enums.len()),
            enums_poly: HashMap::with_capacity((hir.enums.len() / 8).next_power_of_two()),
            structs: Vec::with_capacity(hir.structs.len()),
            structs_poly: HashMap::with_capacity((hir.structs.len() / 8).next_power_of_two()),
            globals: Vec::with_capacity(hir.globals.len()),
            strings: HashMap::with_capacity(session.intern_lit.get_all().len()),
            hir,
            session,
            namebuf: String::with_capacity(256),
            cache,
            info,
            proc_queue: Vec::with_capacity(64),
            emit: ErrorBuffer::default(),
        }
    }

    pub fn enum_layout(&mut self, key: hir::EnumKey<'c>) -> hir::Layout {
        types::expect_concrete(key.1);
        if let Some(layout) = self.hir.enum_layout.get(&key) {
            return *layout;
        }
        let layout_res = layout::resolve_enum_layout(self, key.0, key.1);
        let layout = layout_res.unwrap_or_else(|_| hir::Layout::new(0, 1));
        self.hir.enum_layout.insert(key, layout);
        layout
    }

    pub fn struct_layout(&mut self, key: hir::StructKey<'c>) -> hir::StructLayout<'c> {
        types::expect_concrete(key.1);
        if let Some(layout) = self.hir.struct_layout.get(&key) {
            return *layout;
        }
        let layout = layout::resolve_struct_layout(self, key.0, key.1).unwrap(); //@default
        self.hir.struct_layout.insert(key, layout);
        layout
    }

    pub fn variant_layout(&mut self, key: hir::VariantKey<'c>) -> hir::StructLayout<'c> {
        types::expect_concrete(key.2);
        if let Some(layout) = self.hir.variant_layout.get(&key) {
            return *layout;
        }
        let _ = self.enum_layout((key.0, key.2));
        *self.hir.variant_layout.get(&key).unwrap() //@default + insert
    }

    pub fn ty(&mut self, ty: hir::Type<'c>) -> llvm::Type {
        self.ty_impl(ty, &[])
    }

    fn ty_impl(&mut self, ty: hir::Type<'c>, poly_set: &[hir::Type<'c>]) -> llvm::Type {
        match ty {
            hir::Type::Error | hir::Type::Unknown => unreachable!(),
            hir::Type::Char => self.cache.int_32,
            hir::Type::Void => self.cache.void_val_type.as_ty(),
            hir::Type::Never => self.cache.void_val_type.as_ty(),
            hir::Type::Rawptr => self.cache.ptr_type,
            hir::Type::UntypedChar => unreachable!(),
            hir::Type::Int(ty) => self.int_type(ty),
            hir::Type::Float(ty) => self.float_type(ty),
            hir::Type::Bool(ty) => self.bool_type(ty),
            hir::Type::String(ty) => self.string_type(ty),
            hir::Type::PolyProc(_, idx) => self.ty(self.proc.poly_types[idx]),
            hir::Type::PolyEnum(_, idx) => self.ty(poly_set[idx]),
            hir::Type::PolyStruct(_, idx) => self.ty(poly_set[idx]),
            hir::Type::Enum(enum_id, poly_types) => {
                if poly_types.is_empty() {
                    return self.enums[enum_id.index()].as_ty();
                }

                let key = (enum_id, substitute_types(self, poly_types, poly_set));
                types::expect_concrete(key.1);
                if let Some(t) = self.enums_poly.get(&key) {
                    return t.as_ty();
                }

                let data = self.hir.enum_data(enum_id);
                self.namebuf.clear();
                write_symbol_name(self, data.name.id, data.origin_id, key.1);
                let opaque = self.context.struct_named_create(&self.namebuf);
                self.enums_poly.insert(key, opaque);

                //@always 1 align, probably generating unaligned memory ops?
                let layout = self.enum_layout(key);
                let array_ty = llvm::array_type(self.cache.int_8, layout.size);
                self.context.struct_named_set_body(opaque, &[array_ty], false);
                opaque.as_ty()
            }
            hir::Type::Struct(struct_id, poly_types) => {
                if poly_types.is_empty() {
                    return self.structs[struct_id.index()].as_ty();
                }

                let key = (struct_id, substitute_types(self, poly_types, poly_set));
                types::expect_concrete(key.1);
                if let Some(t) = self.structs_poly.get(&key) {
                    return t.as_ty();
                }

                let data = self.hir.struct_data(struct_id);
                self.namebuf.clear();
                write_symbol_name(self, data.name.id, data.origin_id, key.1);
                let opaque = self.context.struct_named_create(&self.namebuf);
                self.structs_poly.insert(key, opaque);

                let data = self.hir.struct_data(struct_id);
                let offset = self.cache.types.start();
                for field in data.fields {
                    let ty = self.ty_impl(field.ty, key.1);
                    self.cache.types.push(ty);
                }
                let field_types = self.cache.types.view(offset);
                self.context.struct_named_set_body(opaque, &field_types, false);
                self.cache.types.pop_view(offset);
                opaque.as_ty()
            }
            hir::Type::Reference(_, _) => self.cache.ptr_type,
            hir::Type::MultiReference(_, _) => self.cache.ptr_type,
            hir::Type::Procedure(_) => self.cache.ptr_type,
            hir::Type::ArraySlice(_) => self.cache.slice_type.as_ty(),
            hir::Type::ArrayStatic(array) => {
                llvm::array_type(self.ty_impl(array.elem_ty, poly_set), self.array_len(array.len))
            }
            hir::Type::ArrayEnumerated(array) => {
                let len = self.hir.enum_data(array.enum_id).variants.len() as u64;
                llvm::array_type(self.ty_impl(array.elem_ty, poly_set), len)
            }
        }
    }

    pub fn char_type(&self) -> llvm::Type {
        self.cache.int_32
    }
    pub fn void_type(&self) -> llvm::Type {
        self.cache.void_type
    }
    pub fn void_val_type(&self) -> llvm::TypeStruct {
        self.cache.void_val_type
    }
    pub fn ptr_type(&self) -> llvm::Type {
        self.cache.ptr_type
    }
    pub fn ptr_sized_int(&self) -> llvm::Type {
        self.cache.ptr_sized_int
    }
    pub fn slice_type(&self) -> llvm::TypeStruct {
        self.cache.slice_type
    }

    pub fn int_type(&self, int_ty: hir::IntType) -> llvm::Type {
        match int_ty {
            hir::IntType::S8 | hir::IntType::U8 => self.cache.int_8,
            hir::IntType::S16 | hir::IntType::U16 => self.cache.int_16,
            hir::IntType::S32 | hir::IntType::U32 => self.cache.int_32,
            hir::IntType::S64 | hir::IntType::U64 => self.cache.int_64,
            hir::IntType::Ssize | hir::IntType::Usize => self.cache.ptr_sized_int,
            hir::IntType::Untyped => unreachable!(),
        }
    }
    pub fn float_type(&self, float_ty: hir::FloatType) -> llvm::Type {
        match float_ty {
            hir::FloatType::F32 => self.cache.float_32,
            hir::FloatType::F64 => self.cache.float_64,
            hir::FloatType::Untyped => unreachable!(),
        }
    }
    pub fn bool_type(&self, bool_ty: hir::BoolType) -> llvm::Type {
        match bool_ty {
            hir::BoolType::Bool => self.cache.int_1,
            hir::BoolType::Bool16 => self.cache.int_16,
            hir::BoolType::Bool32 => self.cache.int_32,
            hir::BoolType::Bool64 => self.cache.int_64,
            hir::BoolType::Untyped => unreachable!(),
        }
    }
    pub fn string_type(&self, string_ty: hir::StringType) -> llvm::Type {
        match string_ty {
            hir::StringType::String => self.cache.slice_type.as_ty(),
            hir::StringType::CString => self.cache.ptr_type,
            hir::StringType::Untyped => unreachable!(),
        }
    }

    pub fn proc_type(&mut self, proc_ty: &hir::ProcType<'c>) -> llvm::TypeFn {
        let offset = self.cache.types.start();
        for param in proc_ty.params {
            let ty = self.ty(param.ty);
            self.cache.types.push(ty);
        }
        let return_ty = self.ty(proc_ty.return_ty);
        let is_variadic = proc_ty.flag_set.contains(hir::ProcFlag::CVariadic);

        let param_types = self.cache.types.view(offset);
        let proc_ty = llvm::function_type(return_ty, param_types, is_variadic);
        self.cache.types.pop_view(offset);
        proc_ty
    }

    pub fn array_len(&self, len: hir::ArrayStaticLen) -> u64 {
        match len {
            hir::ArrayStaticLen::Immediate(len) => len,
            hir::ArrayStaticLen::ConstEval(eval_id) => {
                match self.hir.const_eval_values[eval_id.index()] {
                    hir::ConstValue::Int { val, .. } => val,
                    _ => unreachable!(),
                }
            }
        }
    }

    #[inline]
    pub fn append_bb(&mut self, name: &str) -> llvm::BasicBlock {
        self.context.append_bb(self.proc.fn_val, name)
    }
    #[inline]
    pub fn insert_bb_terminated(&self) -> bool {
        self.build.insert_bb().terminator().is_some()
    }
    #[inline]
    pub fn build_br_no_term(&self, bb: llvm::BasicBlock) {
        if !self.insert_bb_terminated() {
            self.build.br(bb);
        }
    }
    #[must_use]
    pub fn entry_alloca(&mut self, ty: llvm::Type, name: &str) -> llvm::ValuePtr {
        let insert_bb = self.build.insert_bb();
        let entry_bb = self.proc.fn_val.entry_bb();

        if let Some(instr) = entry_bb.first_instr() {
            self.build.position_before_instr(instr);
        } else {
            self.build.position_at_end(entry_bb);
        }

        let ptr_val = self.build.alloca(ty, name);
        self.build.position_at_end(insert_bb);
        ptr_val
    }
    #[must_use]
    pub fn position_after(&self, inst: llvm::ValueInstr) -> llvm::BasicBlock {
        let insert_bb = self.build.insert_bb();
        if let Some(next) = self.build.next_inst(inst) {
            self.build.position_before_instr(next);
        } else {
            self.build.position_at_end(self.build.inst_block(inst));
        }
        insert_bb
    }

    #[inline]
    pub fn const_usize(&self, val: u64) -> llvm::Value {
        llvm::const_int(self.ptr_sized_int(), val, false)
    }
}

impl<'c> ProcCodegen<'c> {
    pub fn new() -> ProcCodegen<'c> {
        ProcCodegen {
            proc_id: hir::ProcID::dummy(),
            fn_val: llvm::ValueFn::null(),
            param_ptrs: Vec::with_capacity(16),
            variable_ptrs: Vec::with_capacity(128),
            poly_types: &[],
            tail_values: Vec::with_capacity(32),
            block_stack: Vec::with_capacity(16),
            next_loop_info: None,
        }
    }

    pub fn reset(
        &mut self,
        proc_id: hir::ProcID,
        fn_val: llvm::ValueFn,
        poly_types: &'c [hir::Type<'c>],
    ) {
        types::expect_concrete(poly_types);
        self.proc_id = proc_id;
        self.fn_val = fn_val;
        self.param_ptrs.clear();
        self.variable_ptrs.clear();
        self.poly_types = poly_types;
        self.tail_values.clear();
        self.block_stack.clear();
        self.next_loop_info = None;
    }

    pub fn block_enter(&mut self) {
        let block_info = BlockInfo { loop_info: self.next_loop_info.take() };
        self.block_stack.push(block_info);
    }

    pub fn block_exit(&mut self) {
        self.block_stack.pop();
    }

    pub fn set_next_loop_info(
        &mut self,
        break_bb: llvm::BasicBlock,
        continue_bb: llvm::BasicBlock,
    ) {
        let loop_info = LoopInfo { break_bb, continue_bb };
        self.next_loop_info = Some(loop_info);
    }

    pub fn last_loop_info(&self) -> LoopInfo {
        for block in self.block_stack.iter().rev() {
            if let Some(loop_info) = block.loop_info {
                return loop_info;
            }
        }
        unreachable!()
    }

    pub fn add_tail_value(&mut self) -> TailValueID {
        let value_id = TailValueID::new(self.tail_values.len());
        self.tail_values.push(None);
        value_id
    }

    pub fn tail_value(&self, value_id: TailValueID) -> Option<TailValue> {
        self.tail_values[value_id.index()]
    }

    pub fn set_tail_value(
        &mut self,
        value_id: TailValueID,
        value_ptr: llvm::ValuePtr,
        value_ty: llvm::Type,
    ) {
        let value = TailValue { value_ptr, value_ty };
        self.tail_values[value_id.index()] = Some(value);
    }
}

impl<'c> CodegenCache<'c> {
    fn new(context: &mut llvm::IRContext, target: &llvm::IRTarget) -> CodegenCache<'c> {
        let ptr_type = context.ptr_type();
        let ptr_sized_int = target.ptr_sized_int(context);
        let slice_type = context.struct_named_create("rock.slice");
        let void_val_type = context.struct_named_create("rock.void");
        context.struct_named_set_body(slice_type, &[ptr_type, ptr_sized_int], false);
        context.struct_named_set_body(void_val_type, &[], false);

        CodegenCache {
            int_1: context.int_1(),
            int_8: context.int_8(),
            int_16: context.int_16(),
            int_32: context.int_32(),
            int_64: context.int_64(),
            float_32: context.float_32(),
            float_64: context.float_64(),
            void_type: context.void_type(),
            ptr_type,
            ptr_sized_int,
            slice_type,
            void_val_type,
            zero_i8: llvm::const_int(context.int_8(), 0, false),
            sret: context.attr_create("sret"),
            noreturn: context.attr_create("noreturn"),
            inlinehint: context.attr_create("inlinehint"),
            u8s: TempBuffer::new(32),
            u64s: TempBuffer::new(32),
            types: TempBuffer::new(64),
            values: TempBuffer::new(128),
            const_values: TempBuffer::new(128),
            cases: TempBuffer::new(128),
            hir_types: TempBuffer::new(32),
            hir_proc_ty_params: TempBuffer::new(32),
        }
    }
}

impl<'c> CodegenTypeInfo<'c> {
    fn new() -> CodegenTypeInfo<'c> {
        CodegenTypeInfo {
            var_args: Vec::with_capacity(256),
            type_ids: HashMap::with_capacity(256),
            types: Vec::with_capacity(256),
            enums: Vec::with_capacity(64),
            structs: Vec::with_capacity(64),
            references: Vec::with_capacity(32),
            procedures: Vec::with_capacity(32),
            slices: Vec::with_capacity(32),
            arrays: Vec::with_capacity(32),
            variants: Vec::with_capacity(128),
            variant_fields: Vec::with_capacity(128),
            fields: Vec::with_capacity(128),
            param_types: Vec::with_capacity(64),
            types_id: hir::GlobalID::dummy(),
            enums_id: hir::GlobalID::dummy(),
            structs_id: hir::GlobalID::dummy(),
            references_id: hir::GlobalID::dummy(),
            procedures_id: hir::GlobalID::dummy(),
            slices_id: hir::GlobalID::dummy(),
            arrays_id: hir::GlobalID::dummy(),
            variants_id: hir::GlobalID::dummy(),
            variant_fields_id: hir::GlobalID::dummy(),
            fields_id: hir::GlobalID::dummy(),
            param_types_id: hir::GlobalID::dummy(),
        }
    }
}

impl<'hir> types::SubstituteContext<'hir> for Codegen<'hir, '_, '_> {
    fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.hir.arena
    }
    fn types(&mut self) -> &mut TempBuffer<hir::Type<'hir>> {
        &mut self.cache.hir_types
    }
    fn proc_ty_params(&mut self) -> &mut TempBuffer<hir::ProcTypeParam<'hir>> {
        &mut self.cache.hir_proc_ty_params
    }
    fn take_types(&mut self, offset: TempOffset<hir::Type<'hir>>) -> &'hir [hir::Type<'hir>] {
        self.cache.hir_types.take(offset, &mut self.hir.arena)
    }
    fn take_proc_ty_params(
        &mut self,
        offset: TempOffset<hir::ProcTypeParam<'hir>>,
    ) -> &'hir [hir::ProcTypeParam<'hir>] {
        self.cache.hir_proc_ty_params.take(offset, &mut self.hir.arena)
    }
}

impl<'hir> layout::LayoutContext<'hir> for Codegen<'hir, '_, '_> {
    fn u8s(&mut self) -> &mut TempBuffer<u8> {
        &mut self.cache.u8s
    }
    fn u64s(&mut self) -> &mut TempBuffer<u64> {
        &mut self.cache.u64s
    }
    fn take_u8s(&mut self, offset: TempOffset<u8>) -> &'hir [u8] {
        self.cache.u8s.take(offset, &mut self.hir.arena)
    }
    fn take_u64s(&mut self, offset: TempOffset<u64>) -> &'hir [u64] {
        self.cache.u64s.take(offset, &mut self.hir.arena)
    }

    fn error(&mut self) -> &mut impl crate::error::ErrorSink {
        &mut self.emit
    }
    fn ptr_size(&self) -> u64 {
        self.session.config.target_ptr_width.ptr_size()
    }
    fn array_len(&self, len: hir::ArrayStaticLen) -> Result<u64, ()> {
        Ok(self.array_len(len))
    }
    fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        self.hir.enum_data(id)
    }
    fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        self.hir.struct_data(id)
    }

    fn enum_layout(&self) -> &HashMap<hir::EnumKey<'hir>, hir::Layout> {
        &self.hir.enum_layout
    }
    fn struct_layout(&self) -> &HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>> {
        &self.hir.struct_layout
    }
    fn variant_layout(&self) -> &HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>> {
        &self.hir.variant_layout
    }

    fn enum_layout_mut(&mut self) -> &mut HashMap<hir::EnumKey<'hir>, hir::Layout> {
        &mut self.hir.enum_layout
    }
    fn struct_layout_mut(&mut self) -> &mut HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>> {
        &mut self.hir.struct_layout
    }
    fn variant_layout_mut(
        &mut self,
    ) -> &mut HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>> {
        &mut self.hir.variant_layout
    }
}

pub fn substitute_types<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    types: &'c [hir::Type<'c>],
    poly_set: &[hir::Type<'c>],
) -> &'c [hir::Type<'c>] {
    types::substitute_types(cg, types, poly_set, Some(cg.proc.poly_types))
}

pub fn substitute_type<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    ty: hir::Type<'c>,
    poly_set: &[hir::Type<'c>],
) -> hir::Type<'c> {
    if !types::has_poly_param(ty) {
        return ty;
    }
    types::substitute(cg, ty, poly_set, Some(cg.proc.poly_types))
}

pub fn write_symbol_name(
    cg: &mut Codegen,
    name: NameID,
    origin: ModuleID,
    poly_types: &[hir::Type],
) {
    use std::fmt::Write;
    let module = cg.session.module.get(origin);
    let module_name = cg.session.intern_name.get(module.name_id);
    let package = cg.session.graph.package(module.origin);
    let package_name = cg.session.intern_name.get(package.name_id);
    let symbol_name = cg.session.intern_name.get(name);

    let _ = write!(cg.namebuf, "{package_name}:{module_name}:{symbol_name}");
    if !poly_types.is_empty() {
        cg.namebuf.push('(');
        for ty in poly_types.iter().copied() {
            write_type(cg, ty);
            cg.namebuf.push(',');
        }
        cg.namebuf.pop();
        cg.namebuf.push(')');
    }
}

fn write_type(cg: &mut Codegen, ty: hir::Type) {
    use std::fmt::Write;
    match ty {
        hir::Type::Error => unreachable!(),
        hir::Type::Unknown => unreachable!(),
        hir::Type::Char => cg.namebuf.push_str("char"),
        hir::Type::Void => cg.namebuf.push_str("void"),
        hir::Type::Never => cg.namebuf.push_str("never"),
        hir::Type::Rawptr => cg.namebuf.push_str("rawptr"),
        hir::Type::UntypedChar => unreachable!(),
        hir::Type::Int(int_ty) => cg.namebuf.push_str(int_ty.as_str()),
        hir::Type::Float(float_ty) => cg.namebuf.push_str(float_ty.as_str()),
        hir::Type::Bool(bool_ty) => cg.namebuf.push_str(bool_ty.as_str()),
        hir::Type::String(string_ty) => cg.namebuf.push_str(string_ty.as_str()),
        hir::Type::PolyProc(_, idx) => write_type(cg, cg.proc.poly_types[idx]),
        hir::Type::PolyEnum(_, _) => unreachable!(),
        hir::Type::PolyStruct(_, _) => unreachable!(),
        hir::Type::Enum(enum_id, poly_types) => {
            let data = cg.hir.enum_data(enum_id);
            write_symbol_name(cg, data.name.id, data.origin_id, poly_types);
        }
        hir::Type::Struct(struct_id, poly_types) => {
            let data = cg.hir.struct_data(struct_id);
            write_symbol_name(cg, data.name.id, data.origin_id, poly_types);
        }
        hir::Type::Reference(mutt, ref_ty) => {
            cg.namebuf.push('&');
            if mutt == ast::Mut::Mutable {
                cg.namebuf.push_str("mut ");
            }
            write_type(cg, *ref_ty);
        }
        hir::Type::MultiReference(mutt, ref_ty) => {
            cg.namebuf.push('[');
            cg.namebuf.push('&');
            if mutt == ast::Mut::Mutable {
                cg.namebuf.push_str("mut");
            }
            cg.namebuf.push(']');
            write_type(cg, *ref_ty);
        }
        hir::Type::Procedure(proc_ty) => cg.namebuf.push_str("<proc_ty>"), //@todo
        hir::Type::ArraySlice(slice) => {
            cg.namebuf.push('[');
            if slice.mutt == ast::Mut::Mutable {
                cg.namebuf.push_str("mut");
            }
            cg.namebuf.push(']');
            write_type(cg, slice.elem_ty);
        }
        hir::Type::ArrayStatic(array) => {
            let len = cg.array_len(array.len);
            let _ = write!(&mut cg.namebuf, "[{}]", len);
            write_type(cg, array.elem_ty);
        }
        hir::Type::ArrayEnumerated(array) => {
            let data = cg.hir.enum_data(array.enum_id);
            cg.namebuf.push('[');
            write_symbol_name(cg, data.name.id, data.origin_id, &[]);
            cg.namebuf.push(']');
            write_type(cg, array.elem_ty);
        }
    }
}
