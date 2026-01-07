use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::token::Unsafe;
use syn::{ItemFn, parse_macro_input};

fn last_path_ident_is(attr: &syn::Attribute, name: &str) -> bool {
    attr.path()
        .segments
        .last()
        .map(|s| s.ident == name)
        .unwrap_or(false)
}

#[proc_macro_attribute]
pub fn align16_fn_for_interrupt(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = parse_macro_input!(item as ItemFn);

    let user_sig = f.sig.clone();

    if f.sig.asyncness.is_some() {
        return quote!(compile_error!(
            "align16_fn_for_interrupt: async fn は非対応"
        ))
        .into();
    }
    if f.sig.constness.is_some() {
        return quote!(compile_error!(
            "align16_fn_for_interrupt: const fn は非対応"
        ))
        .into();
    }
    if f.sig.variadic.is_some() {
        return quote!(compile_error!(
            "align16_fn_for_interrupt: variadic は非対応"
        ))
        .into();
    }
    if !f.sig.generics.params.is_empty() {
        return quote!(compile_error!(
            "align16_fn_for_interrupt: ジェネリクス付き fn は非対応"
        ))
        .into();
    }

    // (stack_frame, [error_code]) のみ対応
    let argc = f.sig.inputs.len();
    if argc != 1 && argc != 2 {
        return quote!(compile_error!(
            "align16_fn_for_interrupt: 引数は (InterruptStackFrame) か (InterruptStackFrame, u64) のみ対応"
        ))
        .into();
    }
    let has_error_code = argc == 2;

    // 元の名前と inner 名
    let orig_ident = f.sig.ident.clone();
    let inner_ident = format_ident!("__{}_inner", orig_ident);

    // 元の属性（このマクロ自身は除外）
    let orig_attrs: Vec<syn::Attribute> = f
        .attrs
        .into_iter()
        .filter(|a| !last_path_ident_is(a, "align16_fn_for_interrupt"))
        .collect();

    // wrapper の属性
    let wrapper_attrs = {
        let mut v = Vec::new();
        v.extend(orig_attrs.clone());
        v.push(syn::parse_quote!(#[naked]));
        v
    };

    // ===== inner を作る =====
    // inner: 第1引数を *const T に変換して、先頭で read_unaligned して値に戻す
    let (frame_ident, frame_ty) = {
        use syn::{FnArg, Pat, PatIdent, Type};

        let first = f.sig.inputs.first_mut().unwrap();
        let FnArg::Typed(pat_ty) = first else {
            return quote!(compile_error!(
                "align16_fn_for_interrupt: self 引数は非対応"
            ))
            .into();
        };

        let Pat::Ident(PatIdent { ident, .. }) = &*pat_ty.pat else {
            return quote!(compile_error!(
                "align16_fn_for_interrupt: 第1引数は `name: Type` の形にしてください"
            ))
            .into();
        };

        let ty: Type = (*pat_ty.ty).clone();
        pat_ty.ty = Box::new(syn::parse_quote!(*const #ty));
        (ident.clone(), ty)
    };

    // inner 冒頭で値に戻す（これでユーザの本体は従来どおり stack_frame を値として扱える）
    f.block.stmts.insert(
        0,
        syn::parse_quote! {
            let #frame_ident: #frame_ty = unsafe { core::ptr::read_unaligned(#frame_ident) };
        },
    );

    // inner の ABI は sysv64 に固定（= rdi/rsi…）
    f.sig.ident = inner_ident.clone();
    f.sig.abi = Some(syn::parse_quote!(extern "sysv64"));
    f.attrs = orig_attrs.clone();

    let mut wrapper_sig = user_sig;
    wrapper_sig.ident = orig_ident.clone();
    wrapper_sig.abi = Some(syn::parse_quote!(extern "x86-interrupt"));

    // wrapper の引数型は「ユーザが書いた通り」にしたいので、
    // inner に変換された *const 版から元の型に戻す必要がある → ここは item から複製しておくのが楽
    // ただし今回は簡略化のため「ユーザ側は最初から (InterruptStackFrame, [u64]) を書く」前提で、
    // wrapper_sig.inputs を元の item から取り直してください（実装上は保持しておくのが一番確実です）。

    // ↓ここでは「ユーザが item で書いた inputs を wrapper に使う」想定として、元を保持していたと仮定します。
    // 実装時は f を弄る前に `let user_inputs = f.sig.inputs.clone();` を取って wrapper_sig.inputs = user_inputs; にしてください。

    let asm = if has_error_code {
        quote! {
            core::arch::naked_asm!(
                "mov r11, rsp",
                "and rsp, -16",
                "sub rsp, 16",
                "mov [rsp], r11",

                // error_code は [r11], frame は [r11+8..]
                "mov rsi, [r11]",
                "lea rdi, [r11 + 8]",
                "call {inner}",

                "mov r11, [rsp]",
                "mov rsp, r11",
                // エラーコードを捨ててから戻る
                "add rsp, 8",
                "iretq",
                inner = sym #inner_ident,
            );
        }
    } else {
        quote! {
            core::arch::naked_asm!(
                "mov r11, rsp",
                "and rsp, -16",
                "sub rsp, 16",
                "mov [rsp], r11",

                // frame は [r11..]
                "mov rdi, r11",
                "call {inner}",

                "mov r11, [rsp]",
                "mov rsp, r11",
                "iretq",
                inner = sym #inner_ident,
            );
        }
    };

    let expanded = quote! {
        #f

        #(#wrapper_attrs)*
        #wrapper_sig {
            unsafe { #asm }
        }
    };

    expanded.into()
}
