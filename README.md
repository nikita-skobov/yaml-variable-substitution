# yaml-variable-substitution

## Example

```yml
# file.yml

hello: ${{ 2 }}
my_list:
    - apples
    - bananannanas
    - chcocolateete
    - ${{ my_list.0 }}
    - ${{ ENV:SOMETHING }}
```

yaml variable substitution is a library, not a binary,
but it is designed to be able to read command line arguments
and environment variables, if your binary calls one of the `read_yaml_*` helper functions, and provides a context (in this case, context would be an array of command line arguments), you can substitute the above yaml file. Consider if your program was named `my-yaml-program`, then the following command:

```sh  
SOMETHING="here" my-yaml-program file.yml world
```

Would result in the yaml being substituted into:

```yml
hello: world
my_list:
    - apples
    - bananannanas
    - chcocolateete
    - apples
    - here
```

This library uses [context-based-variable-substitution](https://github.com/nikita-skobov/context-based-variable-substitution), for more explanation about how it works, see that library.

This library just provides a few convenience functions for reading yaml (either from file or from string), and substituting the variables found in the yaml file with the variables provided by your application (either a command line argument, or an environment variable)

For more documentation, see the functions called `read_yaml_*` towards the middle of [src/lib.rs](./src/lib.rs).

## Limitations:

- no support for nested transclusion (yet?). eg: cannot do this:
    ```yml
    my:
        nested: variable
    transclude_here: ${{ my }}
    ```
- comments get transcluded too (unfortunately). eg:
    ```yml
    # ${{ old_var }}
    my: ${{ new_var }}
    ```
  if `old_var` is not in the context, then this will fail to parse.
