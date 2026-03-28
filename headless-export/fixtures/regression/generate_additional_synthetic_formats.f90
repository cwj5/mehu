program generate_additional_synthetic_formats
  use iso_fortran_env, only: int32, real32, real64
  implicit none

  call write_case('headless-export/fixtures/regression/synthetic_4x4.xyz', &
                  'headless-export/fixtures/regression/synthetic_4x4.q', .true., .false.)

  call write_case_with_dims('headless-export/fixtures/regression/synthetic_4x4x2.xyz', &
                            'headless-export/fixtures/regression/synthetic_4x4x2.q', &
                            4_int32, 4_int32, 2_int32, .true., .false.)

  call write_case('headless-export/fixtures/regression/synthetic_4x4_le_f64.xyz', &
                  'headless-export/fixtures/regression/synthetic_4x4_le_f64.q', .true., .true.)

  call write_case('headless-export/fixtures/regression/synthetic_4x4_be_f32.xyz', &
                  'headless-export/fixtures/regression/synthetic_4x4_be_f32.q', .false., .false.)

contains

  subroutine write_case(xyz_path, q_path, little_endian, use_f64)
    character(len=*), intent(in) :: xyz_path, q_path
    logical, intent(in) :: little_endian, use_f64

    call write_case_with_dims(xyz_path, q_path, 4_int32, 4_int32, 1_int32, little_endian, use_f64)
  end subroutine write_case

  subroutine write_case_with_dims(xyz_path, q_path, ni, nj, nk, little_endian, use_f64)
    character(len=*), intent(in) :: xyz_path, q_path
    integer(int32), intent(in) :: ni, nj, nk
    logical, intent(in) :: little_endian, use_f64

    integer(int32), parameter :: ngrids = 1, nq = 5, nqc = 0
    integer :: i, j, k, idx, total
    real(real32), dimension(4) :: meta
    real(real32), allocatable :: rho32(:), rhou32(:), rhov32(:), rhow32(:), rhoe32(:)
    real(real32), allocatable :: x32(:), y32(:), z32(:)
    real(real64), allocatable :: rho64(:), rhou64(:), rhov64(:), rhow64(:), rhoe64(:)
    real(real64), allocatable :: x64(:), y64(:), z64(:)
    character(len=16) :: conv

    total = ni * nj * nk
    allocate(rho32(total), rhou32(total), rhov32(total), rhow32(total), rhoe32(total))
    allocate(x32(total), y32(total), z32(total))

    meta = [0.8_real32, 0.0_real32, 1.0e6_real32, 0.0_real32]

    idx = 0
    do k = 1, nk
      do j = 1, nj
        do i = 1, ni
          idx = idx + 1

          x32(idx) = real(i - 1, real32) / real(max(ni - 1, 1_int32), real32)
          y32(idx) = real(j - 1, real32) / real(max(nj - 1, 1_int32), real32)
          z32(idx) = real(k - 1, real32) / real(max(nk - 1, 1_int32), real32)

          rho32(idx) = 1.0_real32 + 0.05_real32 * real(i - 1, real32) + 0.07_real32 * real(j - 1, real32) + 0.09_real32 * real(k - 1, real32)
          rhou32(idx) = 0.12_real32 * real(i - 1, real32)
          rhov32(idx) = 0.10_real32 * real(j - 1, real32)
          rhow32(idx) = 0.08_real32 * real(k - 1, real32) + 0.02_real32 * real(i - j, real32)
          rhoe32(idx) = 2.6_real32 + 0.03_real32 * real(i + j - 2, real32) + 0.06_real32 * real(k - 1, real32)
        end do
      end do
    end do

    if (little_endian) then
      conv = 'little_endian'
    else
      conv = 'big_endian'
    end if

    if (use_f64) then
      allocate(x64(total), y64(total), z64(total))
      allocate(rho64(total), rhou64(total), rhov64(total), rhow64(total), rhoe64(total))

      x64 = real(x32, real64)
      y64 = real(y32, real64)
      z64 = real(z32, real64)
      rho64 = real(rho32, real64)
      rhou64 = real(rhou32, real64)
      rhov64 = real(rhov32, real64)
      rhow64 = real(rhow32, real64)
      rhoe64 = real(rhoe32, real64)

      open(unit=11, file=xyz_path, form='unformatted', access='sequential', status='replace', action='write', convert=conv)
      write(11) ngrids
      write(11) ni, nj, nk
      write(11) x64, y64, z64
      close(11)

      open(unit=12, file=q_path, form='unformatted', access='sequential', status='replace', action='write', convert=conv)
      write(12) ngrids
      write(12) ni, nj, nk, nq, nqc
      write(12) meta
      write(12) rho64, rhou64, rhov64, rhow64, rhoe64
      close(12)

      deallocate(x64, y64, z64, rho64, rhou64, rhov64, rhow64, rhoe64)
    else
      open(unit=21, file=xyz_path, form='unformatted', access='sequential', status='replace', action='write', convert=conv)
      write(21) ngrids
      write(21) ni, nj, nk
      write(21) x32, y32, z32
      close(21)

      open(unit=22, file=q_path, form='unformatted', access='sequential', status='replace', action='write', convert=conv)
      write(22) ngrids
      write(22) ni, nj, nk, nq, nqc
      write(22) meta
      write(22) rho32, rhou32, rhov32, rhow32, rhoe32
      close(22)
    end if

    print *, 'Wrote ', trim(xyz_path)
    print *, 'Wrote ', trim(q_path)

    deallocate(rho32, rhou32, rhov32, rhow32, rhoe32)
    deallocate(x32, y32, z32)
  end subroutine write_case_with_dims

end program generate_additional_synthetic_formats
