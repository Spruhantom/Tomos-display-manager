Name:           tomos-display-manager
Version:        0.0.6
Release:        1%{?dist}
Summary:        Lightweight Rust display manager and GTK4 greetd greeter
License:        MIT
URL:            https://github.com/Spruhantom/Tomos-display-manager
Source0:        %{url}/archive/refs/heads/main.tar.gz
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
Requires:       gtk4
Recommends:     greetd

%description
TDM discovers installed Wayland and X11 desktop sessions and provides a GTK4
greeter that delegates authentication to greetd and the host PAM configuration.
This is experimental software and does not configure or enable greetd.

%prep
%autosetup -n Tomos-display-manager-%{version}

%build
cargo build --release --locked --bins

%install
install -D -m755 target/release/tdm %{buildroot}%{_bindir}/tdm
install -D -m755 target/release/tdm-greeter %{buildroot}%{_bindir}/tdm-greeter
install -D -m644 README.md %{buildroot}%{_docdir}/%{name}/README.md
install -D -m644 packaging/greetd/config.toml.example %{buildroot}%{_docdir}/%{name}/greetd-config.toml.example
%{_docdir}/%{name}/greeter.conf.example
%{_docdir}/%{name}/POWER-AND-THEMES.md
install -D -m644 packaging/tdm/greeter.conf.example %{buildroot}%{_docdir}/%{name}/greeter.conf.example
install -D -m644 docs/POWER-AND-THEMES.md %{buildroot}%{_docdir}/%{name}/POWER-AND-THEMES.md

%files
%{_bindir}/tdm
%{_bindir}/tdm-greeter
%{_docdir}/%{name}/README.md
%{_docdir}/%{name}/greetd-config.toml.example

%license LICENSE

%changelog
* Sat Oct 31 2026 TomOS contributors <maintainers@tomos.example> - 0.0.6-1
- Initial experimental package.
