use std::collections::HashMap;
use std::mem;

use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};

fn main() {
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

    let var_a = variables.get("a").expect("variable a not efined");
    let a = func_builder.use_var(*var_a);
    let var_b = variables.get("b").expect("variable b not efined");
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
