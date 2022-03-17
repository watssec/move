use crate::{parser::ast::{BinOp_, BinOp}};
use move_ir_types::location::sp;
use core::convert::TryFrom;
use num::Integer;
use num::CheckedSub;
use num::CheckedAdd;
extern crate rand;
use rand::Rng;
use std::collections::BTreeMap;
use crate::typing::core::Context;
use move_ir_types::location::*;


// Mutation Details

pub fn booloperator_mutation(op: BinOp) -> BinOp {
    if op.value == BinOp_::And{
        sp(op.loc, BinOp_::And)
    }else{
        sp(op.loc, BinOp_::Eq)
    }
}

pub fn equal_mutation(op: BinOp) -> BinOp{
    if op.value == BinOp_::Neq{
        sp(op.loc, BinOp_::Eq)
    }else{
        sp(op.loc, BinOp_::Neq)
    }
}

pub fn compareoperator_mutation(op: BinOp) -> BinOp{
    let mut rng = rand::thread_rng();
    let mut mutation_selection = rng.gen_range(0..4);

    let dict = BTreeMap::from([
            (0, BinOp_::Lt),
            (1, BinOp_::Gt),
            (2, BinOp_::Ge),
            (3, BinOp_::Le)]);

    // filter out the mutation which is the same as the original one, generate new mutation until valid
    while op.value == *dict.get(&mutation_selection).unwrap(){
        mutation_selection = rng.gen_range(0..4);
    };
    // match mutation

    match mutation_selection{
        0 => {
            sp(op.loc, BinOp_::Lt)
        }
        1 => {
            sp(op.loc, BinOp_::Gt)
        }
        2 => {
            sp(op.loc, BinOp_::Ge)
        }
        3 => {
            sp(op.loc, BinOp_::Le)
        }
        _ => op
    }
}

pub fn bitoperator_mutation(op: BinOp) -> BinOp{
    let mut rng = rand::thread_rng();
    let mut mutation_selection = rng.gen_range(0..3);

    let dict = BTreeMap::from(
        [(0, BinOp_::BitAnd),
            (1, BinOp_::BitOr),
            (2, BinOp_::Xor), ]);

    // filter out the mutation which is the same as the original one, generate new mutation until valid
    while op.value == *dict.get(&mutation_selection).unwrap(){
        mutation_selection = rng.gen_range(0..3);
    }

    // match mutation

    match mutation_selection{
        0 => {
            sp(op.loc, BinOp_::BitAnd)
        }
        1 => {
            sp(op.loc, BinOp_::BitOr)
        }
        2 => {
            sp(op.loc, BinOp_::Xor)
        }
        _ => op
    }
}
pub fn expression_mutation(context:&mut Context, op: BinOp ) ->BinOp {
    let mut rng = rand::thread_rng();
    let mut mutation_selection = rng.gen_range(0..4);

    let dict = BTreeMap::from(
        [(0, BinOp_::Add),
            (1, BinOp_::Sub),
            (2, BinOp_::Div),
            (3, BinOp_::Mul)]);
    let appendix_message = BTreeMap::from(
        [(0, String::from("Add")),
             (1, String::from("Sub")),
             (2, String::from("Div")),
             (3, String::from("Mul"))
        ]
    );
    // filter out the mutation which is the same as the original one, generate new mutation until valid
    while op.value == *dict.get(&mutation_selection).unwrap(){
        mutation_selection = rng.gen_range(0..4);
    }

    // match mutation

    match mutation_selection{
        0 => {
            context.env.appendix = String::from("Add");
            sp(op.loc, BinOp_::Add)
        }
        1 => {
            context.env.appendix = String::from("Sub");
            sp(op.loc, BinOp_::Sub)
        }
        2 => {
            context.env.appendix = String::from("Div");
            sp(op.loc, BinOp_::Div)
        }
        3 => {
            context.env.appendix = String::from("Mul");
            sp(op.loc, BinOp_::Mul)
            }
        _ => op
    }


}


pub fn constant_mutation<T: Integer + CheckedSub+ CheckedAdd + std::ops::Add<Output = T> + std::ops::Sub<Output = T>+TryFrom<u8>+Copy>(context:&mut Context, value: T) -> T
{
    let one = T::try_from(1).ok().unwrap();
    //check whether it's ok to add the constant value, if no overflow, then sub 1
    let mut mutated_constant = value;
    match value.checked_sub(&one){

        None => {
            match value.checked_add(&one){

                None => {
                    context.env.appendix = String::from("didn't mutate");
                },
                _ => {
                    context.env.appendix = String::from("add 1");
                    mutated_constant = value+one
                },
            }
        },
        _ =>{
            context.env.appendix = String::from("sub 1");
            mutated_constant = mutated_constant-one},
    }
    mutated_constant
}

//env management

pub fn env_insert(context:&mut Context,mutation_type:String,loc:Loc)
{
    context.env.diag_map.insert(loc, mutation_type);
    context.env.mutation_counter.insert(loc,false);
    context.env.moduleIdent.insert(loc, context.current_module.unwrap().clone());
}

