use std::collections::HashMap;
use std::mem;

use cranelift::prelude::*;
use cranelift_module::{DataContext, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};

fn main() {
    add();
    hello();
    branch();
}

// func (a, b) {
//    if (a == b) return 0
//    return 1
// }
fn branch() {
    let mut builder_context = FunctionBuilderContext::new();
    let builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(builder);
    let mut ctx = module.make_context();

    let int = module.target_config().pointer_type();
    ctx.func.signature.params.push(AbiParam::new(int));
    ctx.func.signature.params.push(AbiParam::new(int));
    ctx.func.signature.returns.push(AbiParam::new(int));

    let mut func_builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry_ebb = func_builder.create_ebb();
    func_builder.append_ebb_params_for_function_params(entry_ebb);
    func_builder.switch_to_block(entry_ebb);
    func_builder.seal_block(entry_ebb);

    let params: Vec<String> = vec!["a".to_string(), "b".to_string()];
    let the_return = "c".to_string();
    let variables = declare_variables(int, &mut func_builder, &params, &the_return, entry_ebb);

    let var_a = variables.get("a").expect("variable a not defined");
    let a = func_builder.use_var(*var_a);
    let var_b = variables.get("b").expect("variable b not defined");
    let b = func_builder.use_var(*var_b);

    let condition_val = func_builder.ins().icmp(IntCC::Equal, a, b);

    let else_block = func_builder.create_ebb();
    let merge_block = func_builder.create_ebb();
    func_builder.append_ebb_param(merge_block, int);
    func_builder.ins().brz(condition_val, else_block, &[]);

    let then_return = func_builder.ins().iconst(int, 0);
    func_builder.ins().jump(merge_block, &[then_return]);

    func_builder.switch_to_block(else_block);
    func_builder.seal_block(else_block);
    let else_return = func_builder.ins().iconst(int, 1);
    func_builder.ins().jump(merge_block, &[else_return]);

    func_builder.switch_to_block(merge_block);
    func_builder.seal_block(merge_block);

    let phi = func_builder.ebb_params(merge_block)[0];

    let result = phi;
    let var_c = variables.get("c").expect("variable c not defined");
    func_builder.def_var(*var_c, result);

    let return_var = variables.get(&the_return).unwrap();
    let return_val = func_builder.use_var(*return_var);
    func_builder.ins().return_(&[return_val]);
    func_builder.finalize();

    let id = module
        .declare_function("test", Linkage::Export, &ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();

    module.define_function(id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions();
    let code = module.get_finalized_function(id);

    let f = unsafe { mem::transmute::<_, fn(isize, isize) -> isize>(code) };
    println!("result: {}", f(1, 4));
}

fn looper() {}

// func () {
//     puts("hello world")
// }
fn hello() {
    let mut builder_context = FunctionBuilderContext::new();
    let builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(builder);
    let mut ctx = module.make_context();
    let mut data_ctx = DataContext::new();

    // create data
    data_ctx.define("hello world\0".as_bytes().to_vec().into_boxed_slice());
    let data_id = module
        .declare_data("hello_string", Linkage::Export, true, None)
        .unwrap();
    module.define_data(data_id, &data_ctx).unwrap();
    data_ctx.clear();
    module.finalize_definitions();

    let int = module.target_config().pointer_type();
    ctx.func.signature.returns.push(AbiParam::new(int));

    let mut func_builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry_ebb = func_builder.create_ebb();
    func_builder.append_ebb_params_for_function_params(entry_ebb);
    func_builder.switch_to_block(entry_ebb);
    func_builder.seal_block(entry_ebb);

    let params: Vec<String> = vec![];
    let the_return = "c".to_string();
    let variables = declare_variables(int, &mut func_builder, &params, &the_return, entry_ebb);

    // load data to symbol
    let sym = module
        .declare_data("hello_string", Linkage::Export, true, None)
        .unwrap();
    let sym_local_id = module.declare_data_in_func(sym, &mut func_builder.func);
    let pointer = module.target_config().pointer_type();
    let string_to_print = func_builder.ins().symbol_value(pointer, sym_local_id);

    // call libc
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(int));
    sig.returns.push(AbiParam::new(int));
    let callee = module
        .declare_function("puts", Linkage::Import, &sig)
        .unwrap();
    let local_callee = module.declare_func_in_func(callee, &mut func_builder.func);

    let mut arg_values = Vec::new();
    arg_values.push(string_to_print);

    let call = func_builder.ins().call(local_callee, &arg_values);
    func_builder.inst_results(call);

    // make a default return value
    let result = func_builder.ins().iconst(int, 0);
    let var_c = variables.get("c").expect("variable c not defined");
    func_builder.def_var(*var_c, result);

    let return_var = variables.get(&the_return).unwrap();
    let return_val = func_builder.use_var(*return_var);
    func_builder.ins().return_(&[return_val]);
    func_builder.finalize();

    let id = module
        .declare_function("test", Linkage::Export, &ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();

    module.define_function(id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions();
    let code = module.get_finalized_function(id);

    let f = unsafe { mem::transmute::<_, fn() -> isize>(code) };
    println!("result: {}", f());
}

// func (a, b) {
//     return a + b
// }
fn add() {
    let mut builder_context = FunctionBuilderContext::new();
    let builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(builder);
    let mut ctx = module.make_context();

    let int = module.target_config().pointer_type();
    ctx.func.signature.params.push(AbiParam::new(int));
    ctx.func.signature.params.push(AbiParam::new(int));
    ctx.func.signature.returns.push(AbiParam::new(int));

    let mut func_builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry_ebb = func_builder.create_ebb();
    func_builder.append_ebb_params_for_function_params(entry_ebb);
    func_builder.switch_to_block(entry_ebb);
    func_builder.seal_block(entry_ebb);

    let params: Vec<String> = vec!["a".to_string(), "b".to_string()];
    let the_return = "c".to_string();
    let variables = declare_variables(int, &mut func_builder, &params, &the_return, entry_ebb);

    let var_a = variables.get("a").expect("variable a not defined");
    let a = func_builder.use_var(*var_a);
    let var_b = variables.get("b").expect("variable b not defined");
    let b = func_builder.use_var(*var_b);

    let result = func_builder.ins().iadd(a, b);
    let var_c = variables.get("c").expect("variable c not defined");
    func_builder.def_var(*var_c, result);

    let return_var = variables.get(&the_return).unwrap();
    let return_val = func_builder.use_var(*return_var);
    func_builder.ins().return_(&[return_val]);
    func_builder.finalize();

    let id = module
        .declare_function("test", Linkage::Export, &ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();

    module.define_function(id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions();
    let code = module.get_finalized_function(id);

    let f = unsafe { mem::transmute::<_, fn(isize, isize) -> isize>(code) };
    println!("result: {}", f(40, 2));
}

fn declare_variables(
    int: types::Type,
    builder: &mut FunctionBuilder,
    params: &[String],
    the_return: &str,
    entry_ebb: Ebb,
) -> HashMap<String, Variable> {
    let mut variables = HashMap::new();
    let mut index = 0;

    for (i, name) in params.iter().enumerate() {
        // TODO: cranelift_frontend should really have an API to make it easy to set
        // up param variables.
        let val = builder.ebb_params(entry_ebb)[i];
        let var = declare_variable(int, builder, &mut variables, &mut index, name);
        builder.def_var(var, val);
    }
    let zero = builder.ins().iconst(int, 0);
    let return_variable = declare_variable(int, builder, &mut variables, &mut index, the_return);
    builder.def_var(return_variable, zero);

    variables
}

/// Declare a single variable declaration.
fn declare_variable(
    int: types::Type,
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, Variable>,
    index: &mut usize,
    name: &str,
) -> Variable {
    let var = Variable::new(*index);
    if !variables.contains_key(name) {
        variables.insert(name.into(), var);
        builder.declare_var(var, int);
        *index += 1;
    }
    var
}
