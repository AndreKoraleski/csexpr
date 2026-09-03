//! Writing an S-expression in the representations of RFC 9804 §6.
//!
//! §6 gives an S-expression three representations, and this module has one
//! submodule for each. [`canonical`] writes the unique form of §6.2, which is
//! what a signature or a hash is computed over. [`transport`] wraps that form
//! in the braces of §6.3, so it survives a channel that would disturb raw
//! octets. [`advanced`] writes the form of §6.4, which is meant to be read by
//! a person.
//!
//! Every writer here works iteratively, so writing an S-expression consumes no
//! stack in proportion to how deeply its lists nest.

pub mod canonical;
