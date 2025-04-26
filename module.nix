inputs: {
    config,
    lib,
    pkgs,
    ...
}: let
    inherit (pkgs.stdenv.hostPlatform) system;
    inherit (lib) types;

    cfg = config.programs.hippocampus;
in {
    options = {
        programs.hippocampus = {
            enable = lib.mkOption {
                type = lib.types.bool;
                default = false;
                description = ''
                    Whether to enable configuration for Hippocampus, a spaced repetition system.
                '';
                };

            package = lib.mkPackageOption pkgs "hippocampus" { };

            settings = lib.mkOption {
                type = types.submodule {
                    database_url = lib.mkOption {
                        type = types.nullOr types.str;
                        default = null;
                        description = "The database URL to use";
                    };
                    backup_interval_minutes = lib.mkOption {
                        type = types.nullOr types.int;
                        default = null;
                        description = "The interval between backups in minutes";
                    };
                    backup_count = lib.mkOption {
                        type = types.nullOr types.int;
                        default = null;
                        description = "The number of backups to keep";
                    };
                };
                default = {};
                description = "The settings for hippocampus";
            };
        };
    };
    
    config = lib.mkMerge [
        {
            programs.hippocampus = {
                package = lib.mkDefault inputs.self.packages.${system}.hippocampus;
            };
        }
        (lib.mkIf cfg.enable {
            home.packages = [ cfg.package ];

            xdg.configFile."hippocampus/config.toml".source = (pkgs.formats.toml { }).generate "config.toml" cfg.settings;
        })
    ];
}