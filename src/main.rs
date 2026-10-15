#![allow(internal_features)] // rustc_attrs
#![feature(core_intrinsics)]

#![feature(type_info)]
#![feature(const_heap)]
#![feature(const_trait_impl)]
#![feature(ptr_metadata)]
#![feature(rustc_attrs)]
#![feature(const_try)]

use std::fmt::Write;
use std::any::TypeId;
use std::ptr::DynMetadata;
use std::mem::type_info::{TypeKind, VariantId};
use std::marker::PhantomData;

#[allow(unused)]
enum Personality {
    LoveBug {
        purring: u8,
        kneading: u8,
    },
    PrivateInvestigator {
        skittish: Option<u16>,
        curiosity: f32,
    },
}

enum Color {
    Black,
    White,
    Blue,
    Orange,
}

struct Name(String);
struct Age(u8);

#[allow(unused)]
struct Kitty {
    pedigree: Pedigree,
    personalia: (Name, Age),
    paw_colors: [Color; 4],
    personality: Personality,
}

#[allow(unused)]
struct A {
    best_list: bool,
    number: u32,
    list: Vec<u8>,
    answer: u128,
    kitty: Kitty,
}

/// 40 offset is ok, 16 offset is SIGSEGV
impl std::default::Default for A {
    fn default() -> Self { 
        // Self::X { a: 42, b: Some(42) }
        Self{ 
            best_list: true, 
            number: 0,
            list: vec![1,2,3], 
            answer: 42, 
            kitty: Kitty {
                pedigree: Pedigree::Savannah, 
                personalia: (Name("Arty".to_string()), Age(4)),
                paw_colors: [Color::Blue, Color::Orange, Color::Black, Color::Black],
                personality: Personality::PrivateInvestigator { 
                    skittish: Some(9001), 
                    curiosity: 0.5 
                }
            }
        }
    }
}

#[allow(unused)]
enum Pedigree {
    Ragdol,
    BlueRussian,
    Bengal,
    MaineCoon,
    Savannah,
}

#[derive(Debug)]
enum PrintStep {
    PrintStr(&'static str),
    PrintFmt(PrintFmt),
    PrintEnum(PrintEnum),
    PrintArrayElements(PrintArrayElements),
    IncreaseIndentation,
    DecreaseIndentation,
}

impl PrintStep {
    #[rustc_comptime]
    fn fmt_step(field: TypeId, relative_offset: usize) -> Option<PrintStep> {
        Some(PrintStep::PrintFmt(PrintFmt {
            total_offset_to_field: relative_offset as isize, // TODO + total to parent field
            vtable: field
                .trait_info_of::<dyn std::fmt::Debug>()?
                .get_vtable(),
        }))
    }

    fn execute(&self, t: *const (), writer: &mut IndentableWriter<'_>) {
        match self {
            PrintStep::PrintStr(s) => write!(writer, "{}", s).unwrap(),
            PrintStep::PrintFmt(print_fmt_step) => print_fmt_step.execute(t, writer),
            PrintStep::PrintEnum(print_enum) => print_enum.execute(t, writer),
            PrintStep::PrintArrayElements(print_elements) => print_elements.execute(t, writer),
            PrintStep::IncreaseIndentation => writer.indent_level += 1,
            PrintStep::DecreaseIndentation => writer.indent_level -= 1,
        }
    }
}

struct Printer<T>{
    steps: &'static [PrintStep], 
    _phantom: PhantomData<T>, 
}

impl<T: 'static> std::fmt::Debug for Printer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct(const {TypeId::of::<Self>().name() })
            .field(const {std::any::TypeId::of::<Self>().field(0,1).name()}, &self.steps)
            .finish()
    }
}

impl<T> Printer<T> {
    const fn from_steps(steps: Vec<PrintStep>) -> Printer<T> {
        Printer{
            steps: steps.const_make_global(), 
            _phantom: PhantomData,
        }
    }

    fn execute(&self, to_print: &T) {
        let t: *const T = to_print;
        let (t, _meta) = t.to_raw_parts();

        let mut output = String::new();
        let mut writer = IndentableWriter { 
            indent_level: 0,
            inner: &mut output,
            at_start_of_line: true,
        };
        for step in self.steps {
            step.execute(t, &mut writer);
        }
        eprintln!("{output}");
    }
}

struct IndentableWriter<'a>{
    indent_level: usize, 
    at_start_of_line: bool, 
    inner: &'a mut dyn std::fmt::Write
}

impl<'a> std::fmt::Write for IndentableWriter<'a> {
    fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
        for line in s.split_inclusive('\n') {
            if self.at_start_of_line {
                self.at_start_of_line = false;
                for _ in 0..self.indent_level {
                    // self.inner.write_char('\t')?;
                    self.inner.write_str("    ")?;
                }
            }
            self.inner.write_str(line)?;
            self.at_start_of_line = line.ends_with("\n");
        }
        Ok(())
    }
}


#[derive(Debug)]
struct PrintFmt {
    total_offset_to_field: isize,
    vtable: DynMetadata<dyn std::fmt::Debug>,
}

impl PrintFmt {
    fn execute(&self, t: *const (), writer: &mut dyn std::fmt::Write) {
        // the instance the vtable points to
        // SAFETY: only called by Printer which is generic over T and therefore can only be called on 
        // an instance corresponding to the vtable
        let instance = unsafe { t.byte_offset(self.total_offset_to_field) };
        let debug_trait_obj = unsafe { 
            std::ptr::from_raw_parts::<dyn std::fmt::Debug>(
                instance,
                self.vtable,
            ).as_ref().unwrap() 
        };
        write!(writer, "{:?}", debug_trait_obj).unwrap();
    }
}

/// TODO split in builder/consumer
#[derive(Debug)]
struct PrintEnum {
    total_offset_to_field: isize,
    get_variant_id: fn(*const ()) -> VariantId,
    variants: &'static [(VariantId, &'static [PrintStep])],
}

impl PrintEnum {
    fn execute(&self, t: *const (), writer: &mut IndentableWriter<'_>) {
        let variant_id = {
            let t = unsafe { t.byte_offset(self.total_offset_to_field) };
            let variant_id = (self.get_variant_id)(t);
            variant_id
        };

        // TODO pref, can we do direct indexing? Would be really nice if we could...
        // for that the field_idx needs to be public, but we rather do not do that.
        // We do not want this type use-able outside of reflection. Maybe we can 
        // provide a special type later, or implement Index (though that allows one 
        // to get the field_idx out again...)
        let steps = self.variants
            .iter()
            .find(|(id, _)| *id == variant_id)
            .map(|(_, step)| *step)
            .expect("We have a list of steps for each VariantId");

        for step in steps {
            step.execute(t, writer);
        }
    }
}

#[derive(Debug)]
struct PrintArrayElements {
    len: usize,
    element_size: usize,
    steps: &'static [PrintStep],
}

impl PrintArrayElements {
    fn execute(&self, t: *const (), writer: &mut IndentableWriter<'_>) {
        for i in 0..self.len {
            let offset = i * self.element_size; // element alignment equals element size
            let element = unsafe { t.byte_offset(offset as isize) };
            for step in self.steps {
                step.execute(element, writer);
            }
            if i + 1 < self.len {
                write!(writer, ",\n").unwrap();
            }
        }
    }
}

fn debug<T: 'static>(t: &T) {
    let printer = const {
        let mut steps = Vec::new();
        let type_id = TypeId::of::<T>();
        debug_recurse::<T>(type_id, /*initial offset: */ 0, &mut steps);
        Printer::from_steps(steps)
    };
    dbg!(&printer);
    printer.execute(t)
}

#[derive(Debug)]
#[allow(unused)]
struct DebugInfo(TypeId, &'static str, usize);

#[rustc_comptime]
fn debug_recurse<T>(type_id: TypeId, offset: usize, steps: &mut Vec<PrintStep>) {
    if let Some(fmt_step) = PrintStep::fmt_step(type_id, offset) {
        steps.push(fmt_step);
        return;
    }

    match type_id.info().kind {
        TypeKind::Struct => {
            debug_recurse_struct::<T>(type_id, offset, steps);
        }
        TypeKind::Enum => {
            debug_recurse_enum::<T>(type_id, offset, steps);
        }
        TypeKind::Tuple => {
            debug_recurse_tuple::<T>(type_id, offset, steps);
        }
        TypeKind::Array => {
            debug_recurse_array::<T>(type_id, offset, steps);
        }
        _ => panic!("TODO other type kinds"),
    }
}


#[rustc_comptime]
fn has_named_fields(type_id: TypeId, variant_idx: usize) -> bool {
    if type_id.fields(variant_idx) == 0 {
        return true;
    }
    let field_name = type_id.field(variant_idx, 0).name();
    usize::from_str_radix(field_name, 10).is_err() // TODO add to example FieldId::name() and tests
}

#[rustc_comptime]
fn debug_print_fields<T>(type_id: TypeId, variant_idx: usize, offset: usize, steps: &mut Vec<PrintStep>) {
    let named_fields = has_named_fields(type_id, variant_idx);

    steps.push(PrintStep::PrintStr(if named_fields { "{\n" } else { "(\n" }));
    steps.push(PrintStep::IncreaseIndentation);
    let mut field_idx = 0;
    while field_idx < type_id.fields(variant_idx)  {
        let field = type_id.field(variant_idx, field_idx);
        field_idx += 1;
        if named_fields { 
            steps.push(PrintStep::PrintStr(field.name())); 
            steps.push(PrintStep::PrintStr(": "));
        }
        debug_recurse::<T>(field.type_id(), offset + core::cdbg!(field.offset()), steps);
        steps.push(PrintStep::PrintStr(",\n"));
    }
    steps.push(PrintStep::DecreaseIndentation);
    steps.push(PrintStep::PrintStr(if named_fields { "}" } else { ")" }));
}

#[rustc_comptime]
fn debug_recurse_struct<T>(type_id: TypeId, offset: usize, steps: &mut Vec<PrintStep>) {
    steps.push(PrintStep::PrintStr(type_id.name()));
    steps.push(PrintStep::PrintStr(" "));
    debug_print_fields::<T>(type_id, 0, offset, steps);
}

#[rustc_comptime]
fn debug_recurse_tuple<T>(type_id: TypeId, offset: usize, steps: &mut Vec<PrintStep>) {
    debug_print_fields::<T>(type_id, 0, offset, steps);
}

#[rustc_comptime]
fn debug_recurse_array<T>(type_id: TypeId, offset: usize, steps: &mut Vec<PrintStep>) {
    steps.push(PrintStep::PrintStr("[\n"));
    steps.push(PrintStep::IncreaseIndentation);

    let mut steps_per_element = Vec::new();
    debug_recurse::<T>(type_id.element_ty().unwrap(), offset, &mut steps_per_element);

    steps.push(PrintStep::PrintArrayElements(PrintArrayElements { 
        len: type_id.array_len(), 
        element_size: type_id.element_ty().unwrap().size().unwrap(),
        steps: steps_per_element.const_make_global(),
    }));
    steps.push(PrintStep::DecreaseIndentation);
    steps.push(PrintStep::PrintStr("\n]"));
}

#[rustc_comptime]
fn debug_recurse_enum<T>(type_id: TypeId, offset: usize, steps: &mut Vec<PrintStep>) {
    let mut variants: Vec<(VariantId, &'static [PrintStep])> = Vec::new();

    let mut variant_idx = 0;
    while variant_idx < type_id.variants() {
        let mut variant_steps = Vec::new();
        let variant_id = type_id.variant(variant_idx);
        variant_steps.push(PrintStep::PrintStr(variant_id.name()));

        if type_id.fields(variant_idx) > 0 {
            debug_print_fields::<T>(type_id, variant_idx, offset, &mut variant_steps);
        }

        variants.push((variant_id, variant_steps.const_make_global()));
        variant_idx += 1;
    }

    steps.push(PrintStep::PrintEnum(PrintEnum {
        get_variant_id: type_id.variant_getter(),
        variants: variants.const_make_global(),
        total_offset_to_field: offset as isize,
    }));
}

fn main() {
    let a = A::default();
    debug(&a);
}
