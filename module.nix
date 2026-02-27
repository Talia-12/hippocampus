inputs: {
	config,
	lib,
	pkgs,
	...
}: let
	inherit (pkgs.stdenv.hostPlatform) system;
	inherit (lib) types;

	cfg = config.services.hippocampus;
in {
	options = {
		services.hippocampus = {
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
					options = {
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
				};
				default = {};
				description = "The settings for hippocampus";
			};
		};
	};
	
	config = lib.mkMerge [
		{
			services.hippocampus = {
				package = lib.mkDefault inputs.self.packages.${system}.hippocampus;
			};
		}
		(lib.mkIf cfg.enable {
			home.packages = [ cfg.package ];

			xdg.configFile."hippocampus/config.toml".source = (pkgs.formats.toml { }).generate "config.toml" 
				(lib.filterAttrs (_: v: v != null) cfg.settings);

			systemd.user.services.hippocampus = {
				Unit = {
					Description = "Hippocampus spaced repetition system";
					After = [ "network.target" ];
				};
				Service = {
					ExecStart = "${cfg.package}/bin/hippocampus";
					# No automatic restart to prevent potential data loss
					Restart = "no";
				};
				Install = {
					WantedBy = [ "default.target" ];
				};
			};
		})
	];
}
