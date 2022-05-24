#!/bin/bash

if ! command -v brew &> /dev/null ; then
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

brew install cmake
brew install gcc
brew install jq
brew install pkgconf
brew install llvm@13
brew install freetype
brew install expat