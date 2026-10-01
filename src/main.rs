#![feature(type_info)]
#![feature(const_type_name)]
#![feature(const_heap)]
#![feature(const_trait_impl)]
#![feature(const_destruct)]
#![feature(ptr_metadata)]
#![feature(rustc_attrs)]
#![feature(formatting_options)]

use std::any::TypeId;
use std::ptr::DynMetadata;
use std::mem::type_info::TypeKind;
use std::marker::PhantomData;

struct A {
    best_list: bool,
    list: Vec<u8>,
    answer: u128,
    kittykind: KittyKind,
}

/// 40 offset is ok, 16 offset is SIGSEGV
impl std::default::Default for A {
    fn default() -> Self { 
        Self{ 
            best_list: true, 
            list: vec![1,2,3], 
            answer: 42, 
            kittykind: KittyKind::Ragdol 
        }
    }
}

enum KittyKind {
    Ragdol,
    BlueRussian,
}

#[derive(Debug)]
enum PrintStep {
    PrintStr(&'static str),
    PrintFmt(PrintFmtStep),
}

// TODO does this phantom actually give safety in the presence of variance? 
// no... No way to do that compile time. Does it matter? Maybe? Can runtime 
// panic via TypeId.
struct PrinterBuilder<T>{steps: Vec<PrintStep>, _phantom: PhantomData<T>}

struct Printer<T>{steps: &'static [PrintStep], _phantom: PhantomData<T>}

impl<T: 'static> std::fmt::Debug for Printer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct(const {std::any::type_name::<Self>() })
            .field(const {std::any::TypeId::of::<Self>().field(0,1).name()}, &self.steps)
            .finish()
    }
}

impl<T> Printer<T> {
    fn execute(&self, to_print: &T) {
        let t: *const T = to_print;
        let (t, _meta) = t.to_raw_parts();

        let mut output = String::new();
        for step in self.steps {
            match step {
                PrintStep::PrintStr(s) => output.push_str(s),
                PrintStep::PrintFmt(step) => step.execute(t, &mut output),
            }
        }
        eprintln!("{output}");
    }
}

impl<T> PrinterBuilder<T> {
    const fn build(self) -> Printer<T> {
        Printer{steps: self.steps.const_make_global(), _phantom: self._phantom}
    }
    const fn add_str(&mut self, s: &'static str) {
        self.steps.push(PrintStep::PrintStr(s));
    }
    #[rustc_comptime]
    fn add_fmt_step(&mut self, field: TypeId, relative_field_offset: usize) {
        self.steps.push(PrintStep::PrintFmt(PrintFmtStep {
            total_offset_to_field: relative_field_offset as isize, // TODO + total to parent field
            vtable: field
                .trait_info_of::<dyn std::fmt::Debug>()
                .unwrap() // TODO return None if Debug not supported?
                .get_vtable(),
        }));
    }
}

#[derive(Debug)]
struct PrintFmtStep {
    total_offset_to_field: isize,
    vtable: DynMetadata<dyn std::fmt::Debug>,
}

impl PrintFmtStep {
    fn execute(&self, t: *const (), writer: &mut dyn std::fmt::Write) {
        // the instance the vtable points to
        // SAFETY: only called by Printer which is generic over T and therefore can only be called on 
        // an instance corresponding to the vtable
        dbg!("adding ptr");
        let instance = unsafe { t.byte_offset(self.total_offset_to_field) };

        let mut formatter = std::fmt::Formatter::new(writer, Default::default());
        let debug_trait_obj = unsafe { 
            std::ptr::from_raw_parts::<dyn std::fmt::Debug>(
                instance,
                self.vtable,
            ).as_ref().unwrap() 
        };
        dbg!("execution beginning");
        debug_trait_obj.fmt(&mut formatter).unwrap();
        dbg!("execution done");
    }
}

fn debug<T: 'static>(t: &T) {
    let printer = const {
        let mut printer = PrinterBuilder{steps: Vec::new(), _phantom: PhantomData};
        let kind = TypeId::of::<T>().info().kind; // TODO use fmt::Debug if implemented
        match kind {
            TypeKind::Struct => {
                printer.add_str(std::any::type_name::<T>());
                printer.add_str(": {\n");
                let mut field_idx = 0;
                while field_idx < TypeId::of::<T>().fields(0)  {
                    let field = TypeId::of::<T>().field(0, field_idx);
                    field_idx += 1;
                    printer.add_str("\t");
                    printer.add_str(field.name());
                    printer.add_str(": ");
                    match field.type_id().info().kind {
                        TypeKind::Struct => printer.add_fmt_step(field.type_id(), field.offset()),
                        _ => (), // TODO handle other ty-kinds
                    }
                    printer.add_str("\n");
                }
            }
            _ => panic!(),
        }
        printer.add_str("}");
        printer.build()
    };
    dbg!(&printer);
    printer.execute(t)
}


/// TODO add const blocks everywhere
fn dbg<T: 'static>(t: &T) {
    let name = const {
        let id = TypeId::of::<T>();
        id.field(0, 0).name()
    };
    eprint!("{}: ", name);
    dbg!(name);

    let (list_offset, vtable) = const {
        let id = TypeId::of::<T>();
        let list = id.field(0, 1);

        let vtable = list
            .type_id()
            .trait_info_of::<dyn std::fmt::Debug>()
            .expect("Vec<u8> implements Debug")
            .get_vtable();
        (list.offset(), vtable)
    };

    dbg!(&list_offset, &vtable);

    let t: *const T = t;
    let (t, _meta) = t.to_raw_parts();
    let list_value = unsafe { t.byte_offset(list_offset as isize) };
    dbg!(&list_value);

    let mut out = String::new();
    let mut formatter = std::fmt::Formatter::new(&mut out, Default::default());
    let debug_trait_obj = unsafe { 
        std::ptr::from_raw_parts::<dyn std::fmt::Debug>(
            list_value,
            vtable,
        ).as_ref().unwrap() 
    };
    dbg!();
    debug_trait_obj.fmt(&mut formatter).unwrap();
    eprintln!("{}", out);
}

fn main() {
    let a = A::default();
    // dbg(&a);
    debug(&a);
}
