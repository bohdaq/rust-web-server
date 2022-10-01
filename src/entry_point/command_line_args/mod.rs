#[cfg(test)]
mod tests;

pub struct CommandLineArgument {
    short_form: String,
    long_form: String,
    value: String,
}

impl CommandLineArgument {
    pub fn parse(args: Vec<String>) -> Vec<CommandLineArgument> {
        let mut argument_list : Vec<CommandLineArgument> = vec![];
        for unparsed_argument in args.iter() {
            println!("{}", unparsed_argument);
        }
        argument_list
        // fn findArgument(long_form: &str, short_form: &str, args: Vec<String>) -> Option<CommandLineArgument> {
        //     let mut argument = None;
        //     let arg = args.iter().find(|arg| {
        //         let boxed_split = arg.split_once('=');
        //         if boxed_split.is_some() {
        //             let (parameter, value) = boxed_split.unwrap();
        //             let is_parameter_found = parameter.eq(long_form) || parameter.eq(short_form);
        //             if is_parameter_found {
        //                 return true
        //             }
        //         }
        //
        //         false
        //     });

        // }
    }
}
