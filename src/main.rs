use std::any::TypeId;
use std::ptr;

struct A {
    list: Vec<u8>,
}

/// TODO add const blocks everywhere
const fn dbg<T>(t: &T) {
    let id = TypeId::of::<T>();
    eprint!("{}: "id.fields(0, 0).name());
    let list = id.fields(0, 0);

    let debug_trait = None;
    for list_trait in list.type_id().traits() {
        if list_trait == TypeId::of::<Debug>() {
            debug_trait = Some(list_trait)
        }
    }

    // Could also just do this:
    let debug_trait = TypeId::of::<T>();

    let debug_fmt_method = None;
    for method in debug_trait.methods() {
        if method.name() == "fmt" {
            debug_fmt_method = Some(method);
        }
    }
    let debug_fmt_method = debug_fmt_method.unwrap();

    let formatter = Formatter::new() // unstable/nightly unrelated ignore details
    
    // probably not correct/disaster
    let t: *const T = t;
    let t = t.to_raw_parts();
    let list_value = t.offset(list.offset());

    // No idea if this wl even remotely work...
    debug_fmt_method.call([ArgTy::Ref(list_value), ArgTy::RefMut(ptr::addr_of!(formatter))]);
}

struct MethodId {
    trait_: TypeId,
    function: Function,
}

impl MethodId {
    fn method_on(&self) -> TypeId {
        self.function.args[0]
    }
}

struct Function {
    addr: *const (),
    /// if this is a method the first argument is the type of Self
    params: &'static [TypeId],
    ret: TypeId, 
}

impl Function {
    fn call<const N: usize>(&self, args: [FunctionArg; N]) -> Box<dyn Any> {
        todo!("compiler magic")
    }
}

impl MethodId {
    fn call<const N: usize>(&self, args: [FunctionArg; N]) -> Box<dyn Any> {
        self.function.call(args)
    }
}

enum MethodArg {
    Ref(*const ()), // basically a void ptr
    RefMut(*const ()),
    Stack(),
}

fn main() {
    let a = A { list: vec![1,2,3] };
    dbg(a);


}
