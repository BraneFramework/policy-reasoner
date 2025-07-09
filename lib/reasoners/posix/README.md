POSIX Reasoner
==============

A policy reasoner implementation based on POSIX file permissions.

This documentation is aimed at developers that want to maintain or extend the POSIX reasoner. High level documentation
for users of the POSIX reasoner can be found in the [Brane user guide](https://braneframework.github.io/manual). An
explanation of the POSIX policy file can also be found there.

## Goal

This policy reasoner is meant to be easy and widely applicable. The aim is to take few assumptions and require as little
configuration as possible, allowing this reasoner to function as an easy to deploy proof of concept. This allows users
of Brane to gather experience with the abstract concept of a policy reasoner, before one has to start writing policies
themselves.

Additionally, it could function well as an initial reasoner as a user adopts Brane on their systems, since it could use
permissions already set on their current systems to infer which users have access to what data.

## Current permission model

The current permission model is based on the POSIX file permissions. This means that we check if the user has the
required permissions on the file. This is done by checking the file permissions of the file itself, and checking if the
user is either the owner of the file, in the group of the file, or if the file is world readable. The uid and the gids
extracted from the policy are matched against the file's uid and gid. If the file is owned by the user, the owner
permissions are checked. If the file is owned by a group the user is in, the group permissions are checked. If neither
of these is true, the other permissions are checked. If the user has the required permissions, the request is approved.
If not, the request is denied.

## Limitations

Another limitation is that the current implementation is not fully POSIX compliant. We still need to figure out how some
of the POSIX permission behaviours map into this emulation. E.g., right now we only check the file permissions on the
file itself, we do not check the permissions on the directory. Since we are going to be working with network shares (and
possible hard/symlinks) this becomes non-trivial, a working implementation is needed to investigate what behaviour is
desired. This is compounded by the problem that not only the user needs to be able to access the data, but also the
policy reasoner needs to reach at least the directory in which the file resides in order for the reasoner to be able to
`stat(1)` the file.
