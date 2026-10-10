fn main() -> Result<(), skillet_workstation::secrets::CheckError> {
    print!("{}", skillet_workstation::secrets::documentation()?);
    Ok(())
}
