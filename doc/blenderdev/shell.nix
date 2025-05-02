let pkgs = import <nixpkgs> { };
in                                                                         
let                                                                        
  packageOverrides = pkgs.callPackage ./python-packages.nix {};
  python = pkgs.python3.override { inherit packageOverrides; };
	
	python3Packages = pkgs.python3Packages.overrideScope packageOverrides;
	pythonPackages = with python3Packages; [ debugpy flask requests werkzeug click blinker itsdangerous ];
  pythonWithPackages = python.withPackages(_: pythonPackages );
  
	sitePaths = builtins.concatStringsSep ":" (map (pkg:
    "${pkg}/lib/${python.libPrefix}/site-packages"
  ) pythonPackages);
in
pkgs.mkShell {    
  buildInputs = [                       
    pythonWithPackages
  ];

  shellHook = ''
    export PYTHONPATH=${sitePaths}:$PYTHONPATH
    echo "PYTHONPATH: $PYTHONPATH"
  '';
}
