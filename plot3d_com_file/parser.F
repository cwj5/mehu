
      BLOCK DATA BMODE
C
C   Initialize /MODE/, first referenced in SETMOD and SETECH.
C
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
      DATA BATCH/.TRUE./,ECHO/.TRUE./,PRMPT/.TRUE./
      END
      BLOCK DATA BPARIO
C
C   Initialize /PARIO/ and /PARIOS/, first referenced in INITIO.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
      DATA IOINP,IOOUT,IOJOU/0,0,0/
      DATA NINSTK,NOUSTK/0,0/
      END
      BLOCK DATA BPARPR
C
C   Initialize /PARPRM/ and /PARPST/, first referenced in PSHPRM.
C
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
      PARAMETER (MSTACK=20)
      COMMON /PARPST/ NPRSTK,LPRSTK(MSTACK)
      COMMON /PARPSC/ PRMSTK(MSTACK)
      CHARACTER*20 PRMSTK
      DATA PRMCHA/'?'/,LPRMCH/1/
      DATA NPRSTK/0/
      END
      SUBROUTINE CHAARG(NEXT,STRING,STATUS)
C
C   Return this or the next argument as a character string.
C
C      STATUS=1 - No more arguments.
C
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
      LOGICAL NEXT
      CHARACTER*(*) STRING
      INTEGER STATUS
C
      STATUS= 0
C
C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXARG.GE.NARG) THEN
            STATUS= 1
            GOTO 10
         ENDIF
         IXARG= IXARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at an '
     C    //'argument.',1)
         STOP 'CHAARG'
      ENDIF
C
C   Move the argument into STRING.
C
      STRING= ARG(IXARG)
C
   10 CONTINUE
      RETURN
      END
      SUBROUTINE CHAQAR(NEXT,IQ,STRING,STATUS)
C
C   Copy this or the next qualifier argument into the string STRING.
C
C      STATUS=1 - No more arguments.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      LOGICAL NEXT
      CHARACTER*(*) STRING
      INTEGER STATUS
C
      STATUS= 0
C
C   Make sure we're pointing at a qualifier.
C
      IF (IQ.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a'
     C        //'qualifier.', 1)
         STOP 'CHAQAR'
      ENDIF
C
C   If this is a different qualifier, reset the argument pointer and the
C   qualifier pointer.
C
      IF (IQ.NE.IXQUAL) THEN
         IXQARG= 0
         IXQUAL= IQ
      ENDIF
C
C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXQARG.GE.NQARG(2,IQ)) THEN
            STATUS= 1
            GOTO 10
         ENDIF
         IXQARG= IXQARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXQARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier'
     C    //' argument.',1)
         STOP 'CHAQAR'
      ENDIF
C
C   Move the qualifier argument into STRING.
C
      I= NQARG(1,IQ)+IXQARG-1
      STRING= QARG(I)
C
   10 CONTINUE
      RETURN
      END
      SUBROUTINE CHAQUA(NEXT,STRING,STATUS)
C
C   Copy this or the next qualifier into the string STRING.
C
C      STATUS=1 - No more qualifiers.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQUA/ NQUAL
      COMMON /PARQUC/ QUAL(MQUAL)
      CHARACTER*80 QUAL
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      LOGICAL NEXT
      CHARACTER*(*) STRING
      INTEGER STATUS
C
      STATUS= 0
C
C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXQUAL.GE.NQUAL) THEN
            STATUS= 1
            GOTO 10
         ENDIF
         IXQUAL= IXQUAL+1
         IXQARG= 0
      ENDIF
C
C   Make sure we're pointing at a qualifier.
C
      IF (IXQUAL.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier'
     C    //' argument.',1)
         STOP 'CHAQUA'
      ENDIF
C
C   Move the qualifier into STRING.
C
      STRING= QUAL(IXQUAL)
C
   10 CONTINUE
      RETURN
      END
      SUBROUTINE COMLIN(LINE)
C
C   Return the contents of the last command line, minus the actual command.
C
      COMMON /PARLIN/ LIN
      CHARACTER*255 LIN
C
      CHARACTER*(*) LINE
C
      LINE= LIN
      RETURN
      END
      SUBROUTINE COMMAN(NCLIST,CLIST,ICLIST,STATUS)
C
C   Prompt for and read a command line, and parse it into the command,
C   qualifiers, qualifier arguments, and arguments.
C
C      STATUS=1 - Zero length command line.
C      STATUS=2 - End-of-file on read.
C
      COMMON /PARCOM/ COM
      CHARACTER*255 COM
      COMMON /PARLIN/ LIN
      CHARACTER*255 LIN
C
      CHARACTER*(*) CLIST(NCLIST)
      INTEGER STATUS
      CHARACTER*255 LINE
C
C
      STATUS= 0
C 
C   Read a command line.
C
   10 CONTINUE
      CALL PROMPT(' ',0)
C
C   Read an input line.
C
C
       CALL RDLIN(LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         STATUS= 1
         GOTO 20
      ELSE IF (ISTAT.EQ.2) THEN
         STATUS= 2
         GOTO 20
      ELSE
         CALL PTRIM(LINE,LLINE)
         IF (LLINE.EQ.0) THEN
            STATUS= 1
            GOTO 20
         ENDIF
      ENDIF
C
C   Separate out the command.  ISTAT=2 means go call the system for awhile, in
C   which case LINE is a possible system command.
C
      CALL FNDCOM(LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         GOTO 10
      ELSE IF (ISTAT.EQ.2) THEN
         CALL SYSCOM(LINE)
         GOTO 10
      ENDIF
C
C   Check if we match with any of the commands.
C
      CALL MATCH(.FALSE.,COM,NCLIST,CLIST,ICLIST,ISTAT)
      IF (ISTAT.EQ.1) THEN
         CALL PTRIM(COM,LCOM)
         CALL ERRMSG('Command '//COM(1:LCOM)//' not recognized.',1)
         GOTO 10
      ELSE IF (ISTAT.EQ.2) THEN
         CALL PTRIM(COM,LCOM)
         CALL ERRMSG('Ambiguous command '//COM(1:LCOM),1)
         GOTO 10
      ENDIF
C
C   We're OK.  Save the remaining line.
C
      LIN= LINE
      CALL LTRIM(LIN,LLIN)
C
C   Now separate out the qualifiers and qualifier arguments.
C
      CALL FNDQUA(LINE,ISTAT)
      IF (ISTAT.EQ.1) GOTO 10
C
C   Finally get the arguments.
C
      CALL FNDARG(LINE,ISTAT)
      IF (ISTAT.EQ.1) GOTO 10
C
   20 CONTINUE
      RETURN
      END
      SUBROUTINE CVTINT(STRING,IVAL,STATUS)
C
C   Convert a character string to an integer value.
C
C      STATUS=1 - Conversion error.
C
      CHARACTER*(*) STRING
      INTEGER STATUS
C
      CHARACTER*40 TEMP
C
      STATUS= 0
C
C   Use internal read.
C
      TEMP= STRING
      READ(TEMP,1,ERR=10) IVAL
    1 FORMAT(BN,I40)
      RETURN
C
C   Error in conversion.
C
   10 CONTINUE
      STATUS= 1
      RETURN
      END
      SUBROUTINE CVTREA(STRING,RVAL,STATUS)
C
C   Convert a character string to a real number value.
C
C      STATUS=1 - Conversion error.
C
      CHARACTER*(*) STRING
      INTEGER STATUS
C
      CHARACTER*40 TEMP
C
      STATUS= 0
C
C   Use internal read.
C
      TEMP= STRING
      READ(TEMP,1,ERR=10) RVAL
    1 FORMAT(BN,E40.0)
      RETURN
C
C   Error in conversion.
C
   10 CONTINUE
      STATUS= 1
      RETURN
      END
      SUBROUTINE DELFIL(FILE,DELETE,EXIST)
C
C   Check if FILE already exists and return this in EXIST.  If DELETE=TRUE,
C   delete the file, otherwise leave it.  This routine is meant to overcome
C   the FORTRAN 77 "feature" of bombing if one tries to open a new file and
C   the file already exists.
C
      CHARACTER*(*) FILE
      LOGICAL DELETE,EXIST
C
      CHARACTER*20 FORM
      CHARACTER*80 TEXT(2)
C
      CALL PTRIM(FILE,LFILE)
      INQUIRE(FILE=FILE(1:LFILE),EXIST=EXIST,FORMATTED=FORM)
      IF (EXIST .AND. DELETE) THEN
         CALL PTRIM(FORM,LFORM)
         IF      (FORM(1:LFORM).EQ.'YES'    ) THEN
            FORM= 'FORMATTED'
         ELSE IF (FORM(1:LFORM).EQ.'NO'     ) THEN
            FORM= 'UNFORMATTED'
         ELSE IF (FORM(1:LFORM).EQ.'UNKNOWN') THEN
            FORM= 'FORMATTED'
         ENDIF
         CALL PTRIM(FORM,LFORM)
         CALL GETLUN(IUNIT)
         OPEN(UNIT=IUNIT,FILE=FILE(1:LFILE),STATUS='OLD',
     C        FORM=FORM(1:LFORM),ERR=1010)
         CLOSE(UNIT=IUNIT,STATUS='DELETE',ERR=1020)
         CALL FRELUN(IUNIT)
      ENDIF
C
   10 CONTINUE
      RETURN
C
C   Possible problems...
C
 1010 CONTINUE
      WRITE(TEXT,1011) FILE(1:LFILE),FORM(1:LFORM)
 1011 FORMAT('Error opening file ',A/
     C       'FORM=',A,' for deletion in subroutine DELFIL.')
      CALL ERRMSG(TEXT,2)
      GOTO 10
C
 1020 CONTINUE
      WRITE(TEXT,1021) FILE(1:LFILE)
 1021 FORMAT('Error closing file ',A/
     C       'for deletion in subroutine DELFIL.')
      CALL ERRMSG(TEXT,2)
      GOTO 10
      END
      SUBROUTINE ENDARG(STATUS)
C
C   Make sure we're out of arguments.
C
C      STATUS=1 - Too many arguments.
C
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
C
      INTEGER STATUS
C
      CHARACTER*80 TEXT
C
      STATUS= 0
      IF (IXARG.LT.NARG) THEN
         STATUS= 1
         CALL PTRIM(ARG(IXARG+1),LARG)
         TEXT= 'Too many arguments: '//ARG(IXARG+1)(1:LARG)//' ...'
         CALL ERRMSG(TEXT,1)
      ENDIF
      RETURN
      END
      SUBROUTINE ENDIO(JOUSAV)
C
C   Terminate standard I/O for the parser.  Currently this only involves
C   closing the journal file, saving it or not as the case may be.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
C
      LOGICAL JOUSAV
C
C   Dispose of the journal file, keeping or deleting it.
C
      IF (IOJOU.NE.0) THEN
         IF (JOUSAV) THEN
            CLOSE(UNIT=IOJOU)
            CALL FRELUN(IOJOU)
         ELSE
            CLOSE(UNIT=IOJOU,STATUS='DELETE')
         ENDIF
         IOJOU= 0
      ENDIF
      RETURN
      END
      SUBROUTINE ENDQAR(IQ,STATUS)
C
C   Make sure we're out of qualifier arguments.
C
C      STATUS=1 - Too many qualifier arguments.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
C
      INTEGER STATUS
C
      CHARACTER*80 TEXT
C
      STATUS= 0
C
C   If this is a different qualifier, reset the argument pointer and the
C   qualifier pointer.
C
      IF (IQ.NE.IXQUAL) THEN
         IXQARG= 0
         IXQUAL= IQ
      ENDIF
C
      IF (IXQARG.LT.NQARG(2,IXQUAL)) THEN
         STATUS= 1
         CALL PTRIM(QARG(IXQARG+1),LQARG)
         TEXT= 'Too many qualifier arguments: '//QARG(IXQARG+1)(1:LQARG)
     C         //' ...'
         CALL ERRMSG(TEXT,1)
      ENDIF
      RETURN
      END
      SUBROUTINE ERRMSG(TEXT,NLINES)
C
C   Write an error message to the output device.  If this is not the standard
C   output device (i.e. terminal or batch log file), write the message there
C   too.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
C
      CHARACTER*(*) TEXT(*)
C
      CHARACTER*132 TEMP
C
C   Trim it and write it out.
C
      IF (NLINES.GT.0) THEN
         TEMP= '  Error:  '//TEXT(1)
         CALL PTRIM(TEMP,LTEMP)
         CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
         DO 10 I= 2,NLINES
            TEMP= '          '//TEXT(I)
            CALL PTRIM(TEMP,LTEMP)
            CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
   10       CONTINUE
C
C   Temporarily reset the output unit number to zero, indicating the standard
C   output device.
C
         IF (IOOUT.NE.0) THEN
            IOSAV= IOOUT
            IOOUT= 0
            TEMP= '  Error:  '//TEXT(1)
            CALL PTRIM(TEMP,LTEMP)
            CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
            DO 20 I= 2,NLINES
               TEMP= '          '//TEXT(I)
               CALL PTRIM(TEMP,LTEMP)
               CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
   20          CONTINUE
            IOOUT= IOSAV
         ENDIF
C
      ENDIF
C
C   For a batch job, an error is considered fatal (ouch!).  For an interactive
C   job, reset the input and output file stack (don't take any more input from
C   command files).
C
      IF (BATCH) THEN
         STOP 'ERRMSG'
      ELSE
         CALL RSTINP
         CALL RSTOUT
      ENDIF
      RETURN
      END
      SUBROUTINE FNDARG(LINE,STATUS)
C
C   Find all arguments on the command line and put them in ARG.
C
C      STATUS=1 - Syntax error in arguments.
C
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      LOGICAL QUOTE,SUBQUO
      CHARACTER*80 TEXT
C
      STATUS= 0
C
C   Get line length.
C
      CALL PTRIM(LINE,LLINE)
C
C   Start with no arguments.
C
      IXARG= 0
      DO 10 NARG= 1,MARG
         ARG(NARG)= ' '
   10    CONTINUE
C
C   Start search.
C
      NARG= 0
      IE= 0
  100    CONTINUE
C
C   Look for an argument.
C
         NCOMMA= 0
         DO 110 I= IE+1,LLINE
C
C   If it's a blank, keep looking.
C
            IF (LINE(I:I).EQ.' ') THEN
C
C   If it's a comma, make sure we're not getting too many of 'em, and keep
C   looking.
C
            ELSE IF (LINE(I:I).EQ.',') THEN
               NCOMMA= NCOMMA+1
               LINE(I:I)= ' '
C
C   Two commas in a row, or no argument before the first comma.
C
               IF (NARG.EQ.0 .OR. NCOMMA.GT.1) THEN
                  CALL ERRMSG('No argument between commas.',1)
                  STATUS= 1
                  GOTO 400
               ENDIF
C
C   If it's the start of a quote, mark it appropriately.
C
            ELSE IF (LINE(I:I).EQ.'"') THEN
               QUOTE= .TRUE.
               IS= I+1
               LINE(I:I)= ' '
               GOTO 200
C
C   If it's some other nonblank character, it's the start of an argument.  Go
C   look for the end.
C
            ELSE
               QUOTE= .FALSE.
               IS= I
               GOTO 200
            ENDIF
  110       CONTINUE
C
C   End of the arguments.
C
         GOTO 400
C
C   Look for the end of the argument.  The end won't come in the middle of a
C   quote or subquote.  (A quote is just a character argument, enclosed in 
C   quotes to allow embedded blanks or commas.  A subquote may or may not be 
C   inside a quote but is embedded in a character argument.  An example of a 
C   subquote is the "username password" string in the VAX/VMS file specifier 
C
C      node"username password"::disk:[directory]filename
C
  200    CONTINUE
         SUBQUO= .FALSE.
         DO 210 I= IS,LLINE
            IF (SUBQUO) THEN
               IF (LINE(I:I).EQ.'"') SUBQUO= .FALSE.
            ELSE IF (QUOTE) THEN
               IF (LINE(I:I).EQ.'"') THEN
                  IF (I.EQ.LLINE) THEN
                     QUOTE = .FALSE.
                     IE= I-1
                     LINE(I:I)= ' '
                     GOTO 300
                  ELSE IF (LINE(I+1:I+1).EQ.' ' 
     C                .OR. LINE(I+1:I+1).EQ.',') THEN
                     QUOTE = .FALSE.
                     IE= I-1
                     LINE(I:I)= ' '
                     GOTO 300
                  ELSE
                     SUBQUO= .TRUE.
                  ENDIF
               ENDIF
            ELSE IF (LINE(I:I).EQ.'"') THEN
               IF (I.EQ.IS) THEN
                  QUOTE = .TRUE.
                  LINE(I:I)= ' '
               ELSE IF (LINE(I-1:I-1).EQ.' ' .OR. LINE(I-1:I-1).EQ.',')
     C          THEN
                  QUOTE = .TRUE.
                  LINE(I:I)= ' '
               ELSE
                  SUBQUO= .TRUE.
               ENDIF
            ELSE IF (LINE(I:I).EQ.' ' .OR. LINE(I:I).EQ.',') THEN
               IE= I-1
               LINE(I:I)= ' '
               GOTO 300
            ENDIF
  210       CONTINUE
C
C   If we get to the end, then this is the end of the argument.
C
         IE= LLINE
C
C   Got a complete argument.  Check it a little, then store it.
C
  300    CONTINUE
         LARG= IE-IS+1
         NARG= NARG+1
C
C   Check for more arguments than we can save.
C
         IF (NARG.GT.MARG) THEN
            WRITE(TEXT,301) MARG
  301       FORMAT('Number of arguments exceeds internal dimension ',
     C       'limit of ',I5)
            CALL ERRMSG(TEXT,1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Check for a lousy length.
C
         IF (LARG.LE.0) THEN
            CALL ERRMSG('Zero length argument.',1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Check if the argument is too long.
C
         IF (LARG.GT.LEN(ARG(NARG))) THEN
            CALL ERRMSG('Argument length exceeds internal storage '
     C       //'capacity.',1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Store the argument and blank that part of LINE.
C
         ARG(NARG)= LINE(IS:IE)
         LINE(IS:IE)= ' '
C
C   Go back and look for another.
C
         GOTO 100
C
  400 CONTINUE
      RETURN
      END
      SUBROUTINE FNDCOM(LINE,STATUS)
C
C   Find the command in the line and put it in COM.
C
C      STATUS=1 - Error in command line.
C      STATUS=2 - LINE contains system command.
C
      COMMON /PARCOM/ COM
      CHARACTER*255 COM
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
C
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      STATUS= 0
C
C   Command starts as a blank.
C
      COM= ' '
C
C   If the command line starts with the prompt string, blank it out.  (Ignore
C   trailing blanks on the prompt string.)
C
      IF (LPRMCH.NE.0) THEN
         CALL PTRIM(PRMCHA,LLPRM)
         IF (LLPRM.NE.0) THEN
            IPRM= INDEX(LINE,PRMCHA(1:LLPRM))
            IF (IPRM.EQ.1) THEN
               LINE(IPRM:IPRM+LLPRM-1)= ' '
            ELSE IF (IPRM.GT.1) THEN
               IF (LINE(1:IPRM-1).EQ.' ') THEN
                  LINE(IPRM:IPRM+LLPRM-1)= ' '
               ENDIF
            ENDIF
         ENDIF
      ENDIF
C
C   Now trim leading and trailing blanks and get line length.
C
      CALL LTRIM(LINE,LLINE)
C
C   Nothing on the line???  This is actually acceptable, and is just returned
C   as a blank.
C
      IF (LLINE.EQ.0) THEN
C
C   If the first nonblank character is a '$', take this as a system command.
C   Trim off the '$' and leave the rest in LINE.
C
      ELSE IF (LINE(1:1).EQ.'$') THEN
         STATUS= 2
         LINE(1:1)= ' '
         CALL LTRIM(LINE,LLINE)
C
C   Assume we have a command.  A blank or slash is good enough to end it.
C
      ELSE
         IB= INDEX(LINE(1:LLINE),' ')
         IS= INDEX(LINE(1:LLINE),'/')
         IF (IB.EQ.0) IB= LLINE+1
         IF (IS.EQ.0) IS= LLINE+1
         LCOM= MIN(IB-1,IS-1)
C
C   Got the command, now check the length and put it in COM.
C
C   Check for a bad length.
C
         IF (LCOM.EQ.0) THEN
            CALL ERRMSG('No command on line.',1)
            STATUS= 1
C
C   Check to see if it's too long.
C
         ELSE IF (LCOM.GT.LEN(COM)) THEN
            CALL ERRMSG('Command length exceeds internal storage '
     C       //'capacity.',1)
            STATUS= 1
C
C   Store the command and blank out LINE.
C
         ELSE
            COM= LINE(1:LCOM)
            LINE(1:LCOM)= ' '
         ENDIF
      ENDIF
C
  400 CONTINUE
      RETURN
      END
      SUBROUTINE FNDQAR(IQ,LINE,STATUS)
C
C   Find all qualifier arguments in the string and put them in QARG.  See
C   FNDQUA for a description of the data structure used.
C
C      STATUS=1 - Syntax error in qualifier arguments.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      LOGICAL QUOTE,SUBQUO
      CHARACTER*80 TEXT
C
      STATUS= 0
C
C   Get line length.
C
      CALL PTRIM(LINE,LLINE)
C
C   Initialize for this qualifier.  Point to the free space, and give the
C   length.  If we were looking at this qualifier, reset the pointer IXQARG.
C
      NQARG(1,IQ)= IFQARG
      NQARG(2,IQ)= 0
      IF (IQ.EQ.IXQUAL) IXQARG= 0
C
C   Start search.  Only arguments are in this string.
C
      IE= 0
  100    CONTINUE
C
C   Look for an argument.
C
         NCOMMA= 0
         DO 110 I= IE+1,LLINE
C
C   If it's a blank, keep looking.
C
            IF (LINE(I:I).EQ.' ') THEN
C
C   If it's a comma, make sure we're not getting too many of 'em, and keep
C   looking.
C
            ELSE IF (LINE(I:I).EQ.',') THEN
               NCOMMA= NCOMMA+1
               LINE(I:I)= ' '
C
C   Two commas in a row, or no argument before the first comma.
C
               IF (NQARG(2,IQ).EQ.0 .OR. NCOMMA.GT.1) THEN
                  CALL ERRMSG('No qualifier argument between commas.',1)
                  STATUS= 1
                  GOTO 400
               ENDIF
C
C   If it's the start of a quote, mark it appropriately.
C
            ELSE IF (LINE(I:I).EQ.'"') THEN
               QUOTE= .TRUE.
               IS= I+1
               LINE(I:I)= ' '
               GOTO 200
C
C   If it's some other nonblank character, it's the start of an argument.  Go
C   look for the end.
C
            ELSE
               QUOTE= .FALSE.
               IS= I
               GOTO 200
            ENDIF
  110       CONTINUE
C
C   End of the arguments.
C
         GOTO 400
C
C   Look for the end of the argument.  The end won't come in the middle of a
C   quote or subquote.  (A quote is just a character argument, enclosed in 
C   quotes to allow embedded blanks or commas.  A subquote may or may not be 
C   inside a quote but is embedded in a character argument.  An example of a 
C   subquote is the "username password" string in the VAX/VMS file specifier 
C
C      node"username password"::disk:[directory]filename
C
  200    CONTINUE
         SUBQUO= .FALSE.
         DO 210 I= IS,LLINE
            IF (SUBQUO) THEN
               IF (LINE(I:I).EQ.'"') SUBQUO= .FALSE.
            ELSE IF (QUOTE) THEN
               IF (LINE(I:I).EQ.'"') THEN
                  IF (I.EQ.LLINE) THEN
                     QUOTE = .FALSE.
                     IE= I-1
                     LINE(I:I)= ' '
                     GOTO 300
                  ELSE IF (LINE(I+1:I+1).EQ.' ' 
     C                .OR. LINE(I+1:I+1).EQ.',') THEN
                     QUOTE = .FALSE.
                     IE= I-1
                     LINE(I:I)= ' '
                     GOTO 300
                  ELSE
                     SUBQUO= .TRUE.
                  ENDIF
               ENDIF
            ELSE IF (LINE(I:I).EQ.'"') THEN
               IF (I.EQ.IS) THEN
                  QUOTE = .TRUE.
                  LINE(I:I)= ' '
               ELSE IF (LINE(I-1:I-1).EQ.' ' .OR. LINE(I-1:I-1).EQ.',')
     C          THEN
                  QUOTE = .TRUE.
                  LINE(I:I)= ' '
               ELSE
                  SUBQUO= .TRUE.
               ENDIF
            ELSE IF (LINE(I:I).EQ.' ' .OR. LINE(I:I).EQ.',') THEN
               IE= I-1
               LINE(I:I)= ' '
               GOTO 300
            ENDIF
  210       CONTINUE
C
C   If we get to the end, then this is the end of the argument.
C
         IE= LLINE
C
C   Got a complete argument.  Check it a little, then store it.
C
  300    CONTINUE
         LQARG= IE-IS+1
C
C   Check if we've got a place to store it.
C
         IF (IFQARG.GT.MQARG) THEN
            WRITE(TEXT,301) MQARG
  301       FORMAT('Number of qualifier arguments exceeds internal ',
     C       'dimension limit of ',I5)
            CALL ERRMSG(TEXT,1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Check for a bad length.
C
         IF (LQARG.LE.0) THEN
            CALL ERRMSG('Zero length qualifier argument.',1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Check if it's too long to store.
C
         IF (LQARG.GT.LEN(QARG(IFQARG))) THEN
            CALL ERRMSG('Qualifier argument length exceeds internal '
     C       //'storage capacity.',1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Store the qualifier argument and blank that part of LINE.
C
         QARG(IFQARG)= LINE(IS:IE)
         IFQARG= IFQARG+1
         NQARG(2,IQ)= NQARG(2,IQ)+1
         LINE(IS:IE)= ' '
C
C   Go back and look for another.
C
         GOTO 100
C
  400 CONTINUE
      RETURN
      END
      SUBROUTINE FNDQUA(LINE,STATUS)
C
C   Find the qualifiers in LINE and put them in QUAL.  Call FNDQAR to put
C   accompanying arguments in QARG.  Note that slashes may be embedded in an
C   argument by enclosing the argument in (double) quotes, and in this case 
C   should NOT be interpreted as the start of a qualifier.
C
C   Data structure is as follows:  qualifiers are picked up and put in QUAL.
C   Arguments to that qualifier are parsed and put in QARG.  NQARG(1,n) will
C   point to the start of the arguments for the nth qualifier.  NQARG(2,n) will
C   be set to the number of arguments for this qualifier.  If there are no
C   arguments, the number of arguments will be set to zero.
C
C      STATUS=1 - Syntax error in qualifiers.
C
      PARAMETER (MQUAL=20)
      COMMON /PARQUA/ NQUAL
      COMMON /PARQUC/ QUAL(MQUAL)
      CHARACTER*80 QUAL
      PARAMETER (MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      LOGICAL QUOTE,SUBQUO
      CHARACTER*80 TEXT
C
      STATUS= 0
C
C   Get line length.
C
      CALL PTRIM(LINE,LLINE)
      IE= 0
C
C   Start with no qualifiers,
C
      DO 10 NQUAL= 1,MQUAL
         QUAL(NQUAL)= ' '
         NQARG(1,NQUAL)= 0
         NQARG(2,NQUAL)= 0
   10    CONTINUE
C
C   ...and no arguments.
C
      IXQUAL= 0
      IXQARG= 0
      IFQARG= 1
      DO 20 NQA= 1,MQARG
         QARG(NQA)= ' '
   20    CONTINUE
C
C   Start searching.
C
      NQUAL= 0
      NQARG(1,1)= 1
  100    CONTINUE
C
C   Look for the (union label) "/", NOT inside double quotes.
C
         IQEND= INDEX(LINE(IE+1:),'/')
C
C   Check for no more qualifiers.
C
         IF (IQEND.EQ.0) GOTO 500
C
C   Check with the quoted strings to make sure it isn't in one.
C
  110       CONTINUE
            IQUOS= INDEX(LINE(IE+1:),'"')
            IF (IQUOS.NE.0) THEN
C
C   We only have a possible conflict if the quote starts before the slash.
C
               IF (IQUOS.LT.IQEND) THEN
C
C   Found a double quote, look for a matching one.  Nothing between these two 
C   will be considered.
C
                  IQUOE= INDEX(LINE(IE+IQUOS+1:),'"')
C
C   If there is no matching double quote, we're done.  Otherwise fix up the 
C   offset of IQUOE.
C
                  IF (IQUOE.EQ.0) GOTO 500
                  IQUOE= IQUOE+IQUOS
C
C   Update IE to point to after the quoted string.
C
                  IE   = IE+IQUOE
C
C   If the quote ends before the slash, update IQEND also, and look for another
C   quote.
C
                  IF (IQUOE.LT.IQEND) THEN
                     IQEND= IQEND+IQUOE
                     GOTO 110
C
C   Otherwise we ignore this slash, and go look for another.
C
                  ELSE
                     GOTO 100
                  ENDIF
               ENDIF
            ENDIF
C
C   Got a slash, point to it, and blank it out.
C
         IE= IE+IQEND
         LINE(IE:IE)= ' '
C
C   Found a "/", now look for the first nonblank character.
C
         DO 120 I= IE+1,LLINE
C
C   Got the start.
C
            IF (LINE(I:I).NE.' ') THEN
               IS= I
               GOTO 200
            ENDIF
  120       CONTINUE
C
C   Nothing after the "/".
C
         CALL ERRMSG('Expecting a qualifier.',1)
         STATUS= 1
         GOTO 500
C
C   Got the start, now find the end.
C
  200    CONTINUE
         DO 210 I= IS,LLINE
C
C   A blank, another slash, or an '=' are enough to signal the end.
C
            IF (LINE(I:I).EQ.' ' .OR. LINE(I:I).EQ.'/'
     C                           .OR. LINE(I:I).EQ.'=') THEN
               IE= I-1
               GOTO 300
            ENDIF
  210       CONTINUE
C
C   No end.
C
         IE= LLINE
C
C   Got the end, check length.
C
  300    CONTINUE
         LQUAL= IE-IS+1
         NQUAL= NQUAL+1
C
C   Check for more qualifiers than we can save.
C
         IF (NQUAL.GT.MQUAL) THEN
            WRITE(TEXT,301) MQUAL
  301       FORMAT('Number of qualifiers exceeds internal dimension ',
     C       'limit of ',I5)
            CALL ERRMSG(TEXT,1)
            STATUS= 1
            GOTO 500
         ENDIF
C
C   Check for a lousy length.
C
         IF (LQUAL.LE.0) THEN
            CALL ERRMSG('Zero length qualifier.',1)
            STATUS= 1
            GOTO 500
         ENDIF
C
C   Check if the qualifier is too long.
C
         IF (LQUAL.GT.LEN(QUAL(NQUAL))) THEN
            CALL ERRMSG('Qualifier length exceeds internal storage '
     C       //'capacity.',1)
            STATUS= 1
            GOTO 500
         ENDIF
C
C   Store the qualifier and blank that part of LINE.
C
         QUAL(NQUAL)= LINE(IS:IE)
         LINE(IS:IE)= ' '
C
C   Look for qualifier arguments.
C
         DO 310 I= IE+1,LLINE
C
C   An '=' signals qualifier arguments.
C
            IF (LINE(I:I).EQ.'=') THEN
               IE= I
               LINE(I:I)= ' '
               GOTO 400
C
C   Any other nonblank character we're not interested in.
C
            ELSE IF (LINE(I:I).NE.' ') THEN
               GOTO 100
            ENDIF
  310       CONTINUE
C
C   Off the end.
C
         GOTO 500
C
C   Got an '='.  Qualifier arguments are coming.
C
  400    CONTINUE
         DO 410 I= IE+1,LLINE
C
C   If it's a blank, keep looking.
C
            IF (LINE(I:I).EQ.' ') THEN
C
C   A left paren means we have an argument string.
C
            ELSE IF (LINE(I:I).EQ.'(') THEN
               NPAREN= 1
               IS= I+1
               LINE(I:I)= ' '
               GOTO 420
C
C   A slash is an error - he didn't give us the arguments we were looking for.
C
            ELSE IF (LINE(I:I).EQ.'/') THEN
               CALL ERRMSG('Expecting qualifier arguments.',1)
               STATUS= 1
               GOTO 500
C
C   Any other character is the start of an argument.
C
            ELSE
               IS= I
               NPAREN= 0
               GOTO 420
            ENDIF
  410       CONTINUE
C
C   Didn't get anything. This is an error too.
C
         CALL ERRMSG('Expecting qualifier arguments.',1)
         STATUS= 1
         GOTO 500
C
C   Look for the end of the argument(s).  The end won't come in the middle of a
C   quote or subquote.  (A quote is just a character argument, enclosed in 
C   quotes to allow embedded blanks or commas.  A subquote may or may not be 
C   inside a quote but is embedded in a character argument.  An example of a 
C   subquote is the "username password" string in the VAX/VMS file specifier 
C
C      node"username password"::disk:[directory]filename
C   
  420    CONTINUE
         QUOTE = .FALSE.
         SUBQUO= .FALSE.
         DO 430 I= IS,LLINE
            IF (SUBQUO) THEN
               IF (LINE(I:I).EQ.'"') SUBQUO= .FALSE.
            ELSE IF (QUOTE) THEN
               IF (LINE(I:I).EQ.'"') THEN
                  IF (I.EQ.LLINE) THEN
                     QUOTE = .FALSE.
                  ELSE IF (LINE(I+1:I+1).EQ.' ' 
     C                .OR. LINE(I+1:I+1).EQ.','
     C                .OR. LINE(I+1:I+1).EQ.'/') THEN
                     QUOTE = .FALSE.
                  ELSE IF (LINE(I+1:I+1).EQ.')' .AND. NPAREN.EQ.1) THEN
                     QUOTE = .FALSE.
                  ELSE
                     SUBQUO= .TRUE.
                  ENDIF
               ENDIF
            ELSE IF (LINE(I:I).EQ.'"') THEN
               IF (I.EQ.1) THEN
                  QUOTE = .TRUE.
               ELSE IF (LINE(I-1:I-1).EQ.' ' 
     C             .OR. LINE(I-1:I-1).EQ.',') THEN
                  QUOTE = .TRUE.
               ELSE
                  SUBQUO= .TRUE.
               ENDIF
C
C   If this is a string of arguments, look for the right paren.  Account for
C   nesting of parentheses.
C
            ELSE IF (NPAREN.NE.0) THEN
               IF (LINE(I:I).EQ.'(') THEN
                  NPAREN= NPAREN+1
               ELSE IF (LINE(I:I).EQ.')') THEN
                  NPAREN= NPAREN-1
                  IF (NPAREN.EQ.0) THEN
                     IE= I-1
                     LINE(I:I)= ' '
                     GOTO 440
                  ENDIF
               ENDIF
C
C   If this ISN'T a list of arguments, a blank or slash is OK.  A comma is an 
C   error.
C
            ELSE
               IF (LINE(I:I).EQ.' ' .OR. LINE(I:I).EQ.'/') THEN
                  IE= I-1
                  GOTO 440
               ELSE IF (LINE(I:I).EQ.',') THEN
                  CALL ERRMSG('Multiple qualifier arguments must be '
     C             //'enclosed in parentheses.',1)
                  STATUS= 1
                  GOTO 500
               ENDIF
            ENDIF
  430       CONTINUE
C
C   Ran off the end.  This is OK if we weren't looking for a paren.
C
         IF (NPAREN.NE.0) THEN
            CALL ERRMSG('Matching right parenthesis not found.',1)
            STATUS= 1
            GOTO 500
         ELSE
            IE= LLINE
         ENDIF
C
C   Found the end.  Call FNDQAR to dispose of the qualifier argument(s).
C
  440    CONTINUE
         CALL FNDQAR(NQUAL,LINE(IS:IE),ISTAT)
         IF (ISTAT.EQ.1) THEN
            STATUS= 1
            GOTO 500
         ENDIF
C
C   We've now totally finished with this qualifier.  Go back for another.
C
         GOTO 100
C
  500 CONTINUE
      RETURN
      END
      SUBROUTINE GETCA(STRING,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a character string argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) STRING,PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL CHAARG(.TRUE.,STRING,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL CHAARG(.TRUE.,STRING,ISTAT)
               IF (ISTAT.NE.0) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETCQ(IQ,STRING,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a character string qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) STRING,PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL CHAQAR(.TRUE.,IQ,STRING,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
 10         CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL CHAQAR(.TRUE.,IQ,STRING,ISTAT)
               IF (ISTAT.NE.0) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETIA(IVAL,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get an integer argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL INTARG(.TRUE.,IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL INTARG(.TRUE.,IVAL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  GOTO 10
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETIQ(IQ,IVAL,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get an integer qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL INTQAR(.TRUE.,IQ,IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL INTQAR(.TRUE.,IQ,IVAL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  GOTO 10
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETISA(IS,IE,II,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get an integer set argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL INTARG(.TRUE.,IS,ISTAT)
      IF (ISTAT.EQ.0) THEN
         CALL INTARG(.TRUE.,IE,ISTAT)
         IF (ISTAT.EQ.0) THEN
            CALL INTARG(.TRUE.,II,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (II.GT.0) THEN
                  IF (IE.LT.IS) II= -II
               ELSE IF (II.LT.0) THEN
                  IF (IS.LT.IE) II= -II
               ELSE
C
C   Zero is not a valid increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, set it to 1.
C
               II= 1
               IF (IE.LT.IS) II= -II
            ELSE
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            IE= IS
            II= 1
         ELSE
            ERROR= .TRUE.
         ENDIF
      ELSE
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL INTARG(.TRUE.,IS,ISTAT)
               IF (ISTAT.EQ.0) THEN
                  CALL INTARG(.TRUE.,IE,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     CALL INTARG(.TRUE.,II,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (II.GT.0) THEN
                           IF (IE.LT.IS) II= -II
                        ELSE IF (II.LT.0) THEN
                           IF (IS.LT.IE) II= -II
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        II= 1
                        IF (IE.LT.IS) II= -II
                     ELSE
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IE= IS
                     II= 1
                  ELSE
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETISQ(IQ,IS,IE,II,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get an integer set qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL INTQAR(.TRUE.,IQ,IS,ISTAT)
      IF (ISTAT.EQ.0) THEN
         CALL INTQAR(.TRUE.,IQ,IE,ISTAT)
         IF (ISTAT.EQ.0) THEN
            CALL INTQAR(.TRUE.,IQ,II,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (II.GT.0) THEN
                  IF (IE.LT.IS) II= -II
               ELSE IF (II.LT.0) THEN
                  IF (IS.LT.IE) II= -II
               ELSE
C
C   Zero is not a valid increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, set it to 1.
C
               II= 1
               IF (IE.LT.IS) II= -II
            ELSE
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            IE= IS
            II= 1
         ELSE
            ERROR= .TRUE.
         ENDIF
      ELSE
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL INTQAR(.TRUE.,IQ,IS,ISTAT)
               IF (ISTAT.EQ.0) THEN
                  CALL INTQAR(.TRUE.,IQ,IE,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     CALL INTQAR(.TRUE.,IQ,II,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (II.GT.0) THEN
                           IF (IE.LT.IS) II= -II
                        ELSE IF (II.LT.0) THEN
                           IF (IS.LT.IE) II= -II
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        II= 1
                        IF (IE.LT.IS) II= -II
                     ELSE
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IE= IS
                     II= 1
                  ELSE
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETLIN(STRING,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a line of text.  Prompt for it with string PRMSG.  If the reply is an
C   end-of-file, print the abort message ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) STRING,PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   If we don't have a prompt message, just return no value.
C
      IF (NLPR.EQ.0) THEN
         STATUS= 1
      ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
         CALL PROMPT(PRMSG,NLPR)
         CALL RDLIN(STRING,ISTAT)
         IF (ISTAT.EQ.1) THEN
            STATUS= 1
         ELSE IF (ISTAT.EQ.2) THEN
            STATUS= 2
            IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
         ENDIF
C
      ENDIF
      RETURN
      END
      SUBROUTINE GETRA(RVAL,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a real number argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL REAARG(.TRUE.,RVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL REAARG(.TRUE.,RVAL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  GOTO 10
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETRQ(IQ,RVAL,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a real number qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   Try the command line.
C
      CALL REAQAR(.TRUE.,IQ,RVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               CALL REAQAR(.TRUE.,IQ,RVAL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  GOTO 10
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) GOTO 10
               ENDIF
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETRSA(RS,RE,RI,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a real number set argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL REAARG(.TRUE.,RS,ISTAT)
      IF (ISTAT.EQ.0) THEN
         CALL REAARG(.TRUE.,RE,ISTAT)
         IF (ISTAT.EQ.0) THEN
            CALL REAARG(.TRUE.,RI,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (RI.GT.0.) THEN
                  IF (RE.LT.RS) RI= -RI
               ELSE IF (RI.LT.0.) THEN
                  IF (RS.LT.RE) RI= -RI
               ELSE
C
C   Zero is not a valid increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   Generally, there should be an increment for a real number set.  However, if
C   the start and end were the same, we don't really need it.
C
               IF (RE.EQ.RS) THEN
                  RI= 1.
               ELSE
                  CALL ERRMSG('An increment is required if '
     C             //'you give both a start and an end.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            RE= RS
            RI= 1.
         ELSE
            ERROR= .TRUE.
         ENDIF
      ELSE
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL REAARG(.TRUE.,RS,ISTAT)
               IF (ISTAT.EQ.0) THEN
                  CALL REAARG(.TRUE.,RE,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     CALL REAARG(.TRUE.,RI,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (RI.GT.0.) THEN
                           IF (RE.LT.RS) RI= -RI
                        ELSE IF (RI.LT.0.) THEN
                           IF (RS.LT.RE) RI= -RI
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        IF (RE.EQ.RS) THEN
                           RI= 1.
                        ELSE
                           CALL ERRMSG('An increment is required if '
     C                      //'you give both a start and an end.',1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     RE= RS
                     RI= 1.
                  ELSE
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETRSQ(IQ,RS,RE,RI,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get a real number set qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL REAQAR(.TRUE.,IQ,RS,ISTAT)
      IF (ISTAT.EQ.0) THEN
         CALL REAQAR(.TRUE.,IQ,RE,ISTAT)
         IF (ISTAT.EQ.0) THEN
            CALL REAQAR(.TRUE.,IQ,RI,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (RI.GT.0.) THEN
                  IF (RE.LT.RS) RI= -RI
               ELSE IF (RI.LT.0.) THEN
                  IF (RS.LT.RE) RI= -RI
               ELSE
C
C   Zero is not a valid increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   Generally, there should be an increment for a real number set.  However, if
C   the start and end were the same, we don't really need it.
C
               IF (RE.EQ.RS) THEN
                  RI= 1.
               ELSE
                  CALL ERRMSG('An increment is required if '
     C             //'you give both a start and an end.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            RE= RS
            RI= 1.
         ELSE
            ERROR= .TRUE.
         ENDIF
      ELSE
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get
C   something intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL REAQAR(.TRUE.,IQ,RS,ISTAT)
               IF (ISTAT.EQ.0) THEN
                  CALL REAQAR(.TRUE.,IQ,RE,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     CALL REAQAR(.TRUE.,IQ,RI,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (RI.GT.0.) THEN
                           IF (RE.LT.RS) RI= -RI
                        ELSE IF (RI.LT.0.) THEN
                           IF (RS.LT.RE) RI= -RI
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        IF (RE.EQ.RS) THEN
                           RI= 1.
                        ELSE
                           CALL ERRMSG('An increment is required if '
     C                      //'you give both a start and an end.',1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     RE= RS
                     RI= 1.
                  ELSE
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETXSA(igrid,IS,IE,II,PRMSG,NLPR,ABMSG,NLAB,
     c ndim,STATUS)
C
C   Get an index set argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL IDXARG(.TRUE.,IS,ISTAT)
          IF(IS.LT.(-1)) IS = NDIM + IS + 1
      IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
         CALL IDXARG(.TRUE.,IE,ISTAT)
          IF(IE.LT.(-1)) IE = NDIM + IE + 1
         IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
            CALL INTARG(.TRUE.,II,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (II.GT.0) THEN
                  IF (  (IE.LT.IS .AND. IE.NE.-1)
     C             .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
               ELSE IF (II.LT.0) THEN
                  IF (  (IS.LT.IE .AND. IS.NE.-1)
     C             .OR. (IE.EQ.-1 .AND. IE.NE.IS)) II= -II
               ELSE
C
C   Can't use a zero increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, set it to 1.
C
               II= 1
               IF (  (IE.LT.IS .AND. IE.NE.-1)
     C          .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
            ELSE IF (ISTAT.EQ.1) THEN
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            IE= IS
            II= 1
         ELSE IF (ISTAT.EQ.3) THEN
C
C   Can't use ALL as an end.
C
            CALL ERRMSG('Can''t use ALL as an end value.',1)
            ERROR= .TRUE.
         ELSE IF (ISTAT.EQ.1) THEN
            ERROR= .TRUE.
         ENDIF
      ELSE IF (ISTAT.EQ.3) THEN
C
C   Using ALL, look for an increment.
C
         CALL INTARG(.TRUE.,II,ISTAT)
         IF (ISTAT.EQ.0) THEN
C
C   Set IS and IE to loop from FIRST to LAST or LAST to FIRST, depending on the
C   sign of II.
C
            IF (II.GT.0) THEN
               IS= 1
               IE= -1
            ELSE IF (II.LT.0) THEN
               IS= -1
               IE= 1
            ELSE
C
C   Can't use a zero increment.
C
               CALL ERRMSG('Zero is NOT a good increment.',1)
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, IS, IE, and II become FIRST, LAST, and 1, 
C   respectively.
C
            IS= 1
            IE= -1
            II= 1
         ELSE IF (ISTAT.EQ.1) THEN
            ERROR= .TRUE.
         ENDIF
      ELSE IF (ISTAT.EQ.1 .OR. ISTAT.EQ.2) THEN
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get something 
C   intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDARG(ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL IDXARG(.TRUE.,IS,ISTAT)
          IF(IS.LT.(-1)) IS = NDIM + IS + 1
               IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
                  CALL IDXARG(.TRUE.,IE,ISTAT)
        IF(IE.LT.(-1)) IE = NDIM + IE + 1
                  IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
                     CALL INTARG(.TRUE.,II,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (II.GT.0) THEN
                           IF (  (IE.LT.IS .AND. IE.NE.-1)
     C                      .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
                        ELSE IF (II.LT.0) THEN
                           IF (  (IS.LT.IE .AND. IS.NE.-1)
     C                      .OR. (IE.EQ.-1 .AND. IE.NE.IS)) II= -II
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        II= 1
                        IF (  (IE.LT.IS .AND. IE.NE.-1)
     C                   .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
                     ELSE IF (ISTAT.EQ.1) THEN
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IE= IS
                     II= 1
                  ELSE IF (ISTAT.EQ.3) THEN
                     CALL ERRMSG('Can''t use ALL as an end value.',1)
                     ERROR= .TRUE.
                  ELSE IF (ISTAT.EQ.1) THEN
                     ERROR= .TRUE.
                  ENDIF
               ELSE IF (ISTAT.EQ.3) THEN
                  CALL INTARG(.TRUE.,II,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     IF (II.GT.0) THEN
                        IS= 1
                        IE= -1
                     ELSE IF (II.LT.0) THEN
                        IS= -1
                        IE= 1
                     ELSE
                        CALL ERRMSG('Zero is NOT a good increment.',1)
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IS= 1
                     IE= -1
                     II= 1
                  ELSE IF (ISTAT.EQ.1) THEN
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDARG(ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE IF (ISTAT.EQ.1) THEN
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE GETXSQ(IQ,IS,IE,II,PRMSG,NLPR,ABMSG,NLAB,STATUS)
C
C   Get an index set qualifier argument by first looking at the command 
C   line.  If it's not there and we have a prompt string PRMSG, prompt for it 
C   from the terminal.  If the reply is an end-of-file, print the abort message
C   ABMSG.  A set is comprised of start, end, and increment values.
C
C      STATUS=1 - No value or carriage return.
C      STATUS=2 - End-of-file.
C
      CHARACTER*(*) PRMSG(*),ABMSG(*)
      INTEGER STATUS
      LOGICAL ERROR
C
      STATUS= 0
C
C   Try the command line.  Use ERROR as a global flag on errors.
C
      ERROR= .FALSE.
      CALL IDXQAR(.TRUE.,IQ,IS,ISTAT)
      IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
         CALL IDXQAR(.TRUE.,IQ,IE,ISTAT)
         IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
            CALL INTQAR(.TRUE.,IQ,II,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (II.GT.0) THEN
                  IF (  (IE.LT.IS .AND. IE.NE.-1)
     C             .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
               ELSE IF (II.LT.0) THEN
                  IF (  (IS.LT.IE .AND. IS.NE.-1)
     C             .OR. (IE.EQ.-1 .AND. IE.NE.IS)) II= -II
               ELSE
C
C   Can't use a zero increment.
C
                  CALL ERRMSG('Zero is NOT a good increment.',1)
                  ERROR= .TRUE.
               ENDIF
            ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, set it to 1.
C
               II= 1
               IF (  (IE.LT.IS .AND. IE.NE.-1)
     C          .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
            ELSE IF (ISTAT.EQ.1) THEN
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If we only get a start, set the end number equal to the start (and dummy 
C   the increment to 1).
C
            IE= IS
            II= 1
         ELSE IF (ISTAT.EQ.3) THEN
C
C   Can't use ALL as an end.
C
            CALL ERRMSG('Can''t use ALL as an end value.',1)
            ERROR= .TRUE.
         ELSE IF (ISTAT.EQ.1) THEN
            ERROR= .TRUE.
         ENDIF
      ELSE IF (ISTAT.EQ.3) THEN
C
C   Using ALL, look for an increment.
C
         CALL INTQAR(.TRUE.,IQ,II,ISTAT)
         IF (ISTAT.EQ.0) THEN
C
C   Set IS and IE to loop from FIRST to LAST or LAST to FIRST, depending on the
C   sign of II.
C
            IF (II.GT.0) THEN
               IS= 1
               IE= -1
            ELSE IF (II.LT.0) THEN
               IS= -1
               IE= 1
            ELSE
C
C   Can't use a zero increment.
C
               CALL ERRMSG('Zero is NOT a good increment.',1)
               ERROR= .TRUE.
            ENDIF
         ELSE IF (ISTAT.EQ.2) THEN
C
C   If there is no increment, IS, IE, and II become FIRST, LAST, and 1, 
C   respectively.
C
            IS= 1
            IE= -1
            II= 1
         ELSE IF (ISTAT.EQ.1) THEN
            ERROR= .TRUE.
         ENDIF
      ELSE IF (ISTAT.EQ.1 .OR. ISTAT.EQ.2) THEN
         ERROR= .TRUE.
      ENDIF
      IF (ERROR) THEN
C
C   No luck.  If we don't have a prompt message, just return no value.
C
         IF (NLPR.EQ.0) THEN
            STATUS= 1
         ELSE
C
C   Prompt for it, and read the input.  Keep prompting until we get something 
C   intelligent.
C
   10       CONTINUE
            CALL PROMPT(PRMSG,NLPR)
            CALL RDQAR(IQ,ISTAT)
            IF (ISTAT.EQ.0) THEN
               ERROR= .FALSE.
               CALL IDXQAR(.TRUE.,IQ,IS,ISTAT)
               IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
                  CALL IDXQAR(.TRUE.,IQ,IE,ISTAT)
                  IF (ISTAT.EQ.0 .OR. ISTAT.GE.4) THEN
                     CALL INTQAR(.TRUE.,IQ,II,ISTAT)
                     IF (ISTAT.EQ.0) THEN
                        IF (II.GT.0) THEN
                           IF (  (IE.LT.IS .AND. IE.NE.-1)
     C                      .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
                        ELSE IF (II.LT.0) THEN
                           IF (  (IS.LT.IE .AND. IS.NE.-1)
     C                      .OR. (IE.EQ.-1 .AND. IE.NE.IS)) II= -II
                        ELSE
                           CALL ERRMSG('Zero is NOT a good increment.',
     C                                 1)
                           ERROR= .TRUE.
                        ENDIF
                     ELSE IF (ISTAT.EQ.2) THEN
                        II= 1
                        IF (  (IE.LT.IS .AND. IE.NE.-1)
     C                   .OR. (IS.EQ.-1 .AND. IS.NE.IE)) II= -II
                     ELSE IF (ISTAT.EQ.1) THEN
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IE= IS
                     II= 1
                  ELSE IF (ISTAT.EQ.3) THEN
                     CALL ERRMSG('Can''t use ALL as an end value.',1)
                     ERROR= .TRUE.
                  ELSE IF (ISTAT.EQ.1) THEN
                     ERROR= .TRUE.
                  ENDIF
               ELSE IF (ISTAT.EQ.3) THEN
                  CALL INTQAR(.TRUE.,IQ,II,ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     IF (II.GT.0) THEN
                        IS= 1
                        IE= -1
                     ELSE IF (II.LT.0) THEN
                        IS= -1
                        IE= 1
                     ELSE
                        CALL ERRMSG('Zero is NOT a good increment.',1)
                        ERROR= .TRUE.
                     ENDIF
                  ELSE IF (ISTAT.EQ.2) THEN
                     IS= 1
                     IE= -1
                     II= 1
                  ELSE IF (ISTAT.EQ.1) THEN
                     ERROR= .TRUE.
                  ENDIF
C                 CALL ENDQAR(IQ,ISTAT)
C                 IF (ISTAT.NE.0) ERROR= .TRUE.
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
               ELSE IF (ISTAT.EQ.1) THEN
                  ERROR= .TRUE.
               ENDIF
               IF (ERROR) GOTO 10
C
            ELSE IF (ISTAT.EQ.1) THEN
               GOTO 10
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               IF (NLAB.NE.0) CALL MSG(ABMSG,NLAB)
            ENDIF
C
         ENDIF
      ENDIF
C
      RETURN
      END
      SUBROUTINE HELP(FILE,LINE)
C
C   Get help on selected topics from Help Library FILE, corresponding to
C   keywords found in LINE.  An interactive mode allows further prompting for
C   subtopics.
C
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
      COMMON /CHPK/ NCHPWD
      EXTERNAL BCHPK
C
      CHARACTER*(*) FILE,LINE
C
      PARAMETER (MKEY=20)
      CHARACTER*80 LEVEL(MKEY),KEYWRD(MKEY),TEMP
      DIMENSION NXTREC(0:MKEY)
C
      DATA NXTREC(0)/1/
C
C   Open Help Library file.
C
      CALL GETLUN(IUNIT)
      IRECL = 80+7*NCHPWD
      OPEN(UNIT=IUNIT,FILE=FILE,STATUS='OLD',FORM='UNFORMATTED',
     C     ACCESS='DIRECT',RECL=IRECL,ERR=1010)
C
C   Use TEMP instead of LINE, so we can change it.
C
      TEMP  = LINE
C
C   If the incoming line is blank, set NKEY=0 to get the default help
C   information first.
C
      NLEVEL= 1
      IF (TEMP.EQ.' ') THEN
         NKEY  = 0
C
C   Parse the incoming line into arguments.  HLPTOP will copy the arguments
C   into KEYWRD and set NKEY.
C
      ELSE
         CALL FNDARG(TEMP,ISTAT)
         CALL HLPTOP(MKEY,NKEY,KEYWRD,LEVEL,NLEVEL,ISTAT)
      ENDIF
C
C   Get help on the current keywords.  Prompt for new ones if we're
C   interactive.  (If ECHO=FALSE, at least the input device is interactive.
C   In this case we can prompt for additional keywords and get more help.)
C
   10    CONTINUE
         CALL HLPENT(NKEY,KEYWRD,LEVEL,NLEVEL,NXTREC,IUNIT,ISTAT)
         IF (ISTAT.EQ.1) THEN
            GOTO 20
         ELSE IF (ISTAT.EQ.2) THEN
            GOTO 1020
         ENDIF
         IF (.NOT.ECHO) THEN
            CALL HLPTOP(MKEY,NKEY,KEYWRD,LEVEL,NLEVEL,ISTAT)
            IF (ISTAT.EQ.1) GOTO 20
            GOTO 10
         ENDIF
C
C   Close the file and we's outta here.
C
   20 CONTINUE
      CLOSE(UNIT=IUNIT)
      CALL FRELUN(IUNIT)
      RETURN
C
C   Error opening Help Library File.
C
 1010 CONTINUE
      TEMP= FILE
      CALL PTRIM(TEMP,LTEMP)
      CALL ERRMSG('Error opening HELP file '//TEMP(1:LTEMP),1)
      GOTO 20
C
C   Error reading Help Library File.
C
 1020 CONTINUE
      TEMP= FILE
      CALL PTRIM(TEMP,LTEMP)
      CALL ERRMSG('Error reading HELP file '//TEMP(1:LTEMP),1)
      GOTO 20
      END
      SUBROUTINE HLPENT(NKEY,KEYWRD,LEVEL,NXTLVL,NXTREC,IUNIT,STATUS)
C
C   Find and print help entries corresponding to the supplied keywords.  On
C   entry, NXTLVL and NXTREC give the next keyword level and next record to
C   search for matches.  On exit, NXTLVL returns the keyword level to prompt
C   for additional keywords.
C
C   If NKEY=0, print the generic text under "HELP" (level 1), and a list of
C   additional topics under level 1.
C
C      STATUS=1 - Exit the HELP facility.
C      STATUS=2 - Error reading HELP file.
C
      CHARACTER*(*) KEYWRD(*),LEVEL(*)
      DIMENSION NXTREC(0:NKEY)
      INTEGER STATUS
C
      CHARACTER*80 KEY,TEXT,TEMP
      INTEGER PRVLVL,UNQLVL
      LOGICAL ELLIPS,WLDCRD,PRTSKP,NOMORE
C
      STATUS= 0
C
C   Initialize output routine MORE.
C
      CALL MORE(' ',0,ISTAT)
      NOMORE= .FALSE.
C
C   Initialize for keyword matching.
C
      PRVLVL= 1
      NLEVEL= NXTLVL
      INEXT = NXTREC(NXTLVL-1)
      ITEXT = 0
      NTEXT = 0
      NEXTR = 0
      PRTSKP= .FALSE.
      MMATCH= 0
      UNQLVL= NXTLVL-1
      IFOUND= 0
C
C   Loop for matching keywords.  Traverse the Help Library tree to the next
C   node (down, horizontal, or up, based on NEXTR).
C
   10    CONTINUE
         CALL HLPTRV(IUNIT,IREC,TEXT,IUP,IDOWN,IPREV,INEXT,ITEXT,
     C               NTEXT,NLEVEL,NEXTR,PRTSKP,ISTAT)
         IF (ISTAT.EQ.1) THEN
            PRVLVL= MIN(NLEVEL,PRVLVL)
         ELSE IF (ISTAT.EQ.2) THEN
            GOTO 20
         ELSE IF (ISTAT.EQ.3) THEN
            STATUS= 2
            GOTO 30
         ENDIF
C
C   Reset PRTSKP, which skips entries in the Help tree that have already been
C   printed.
C
         PRTSKP= .FALSE.
C
C   Get a clean version of the keyword from the Help file...
C
         TEMP  = TEXT
         CALL LTRIM(TEMP,LTEMP)
         LTEMP = MAX(LTEMP,1)
         CALL UPCASE(TEMP(1:LTEMP))
C
C   ...and a clean version of the supplied keyword.
C
         IF (NKEY.EQ.0) THEN
            KEY   = 'HELP'
            LKEY  = 4
            ELLIPS= .FALSE.
            WLDCRD= .FALSE.
         ELSE IF (NLEVEL.LE.NKEY) THEN
            KEY   = KEYWRD(NLEVEL)
            CALL LTRIM(KEY,LKEY)
            LKEY  = MAX(LKEY,1)
            CALL UPCASE(KEY(1:LKEY))
            ELLIPS= .FALSE.
            WLDCRD= .FALSE.
            IF (KEY(1:LKEY).EQ.'*...') THEN
               ELLIPS= .TRUE.
               NKEY = NLEVEL
            ELSE IF (KEY(1:LKEY).EQ.'*') THEN
               WLDCRD= .TRUE.
            ENDIF
         ENDIF
C
C   We have a match.  Add it to our list.
C
         IF (ELLIPS .OR. WLDCRD .OR. TEMP(1:LKEY).EQ.KEY(1:LKEY)) THEN
            LEVEL(NLEVEL) = TEXT
            NXTREC(NLEVEL)= IDOWN
C
C   Keep track of the lowest consecutive unique match.
C
            IF (MMATCH.EQ.0) THEN
               IF (UNQLVL.EQ.NLEVEL-1) THEN
                  UNQLVL= NLEVEL
               ELSE
                  MMATCH= NLEVEL
                  UNQLVL= NLEVEL-1
               ENDIF
            ELSE
               MMATCH= MIN(NLEVEL,MMATCH)
               UNQLVL= MMATCH-1
            ENDIF
C
C   Try to go down one level to match the next keyword.
C
            IF (NLEVEL.LT.NKEY) THEN
               NEXTR = 1
               GOTO 10
C
C   We're at the bottom.  If the last keyword wasn't an ellipsis, print the
C   keywords, text, and list of additional information available.
C
            ELSE IF (.NOT.ELLIPS) THEN
               IFOUND= IFOUND+1
               PRTSKP= .TRUE.
               CALL HLPPRL(LEVEL,PRVLVL,NLEVEL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  NOMORE= .TRUE.
                  GOTO 20
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
                  GOTO 30
               ENDIF
               CALL HLPTXT(NLEVEL,ITEXT,NTEXT,IUNIT,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  NOMORE= .TRUE.
                  GOTO 20
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
                  GOTO 30
               ELSE IF (ISTAT.EQ.3) THEN
                  STATUS= 2
                  GOTO 30
               ENDIF
C
C   If we didn't have any keywords, give additional information for level 1.
C
               IF (NKEY.EQ.0) THEN
                  CALL HLPINF(0,NXTREC(0),IUNIT,ISTAT)
               ELSE
                  CALL HLPINF(NLEVEL,NXTREC(NLEVEL),IUNIT,ISTAT)
               ENDIF
               IF (ISTAT.EQ.1) THEN
                  NOMORE= .TRUE.
                  GOTO 20
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
                  GOTO 30
               ELSE IF (ISTAT.EQ.3) THEN
                  STATUS= 2
                  GOTO 30
               ENDIF
C
C   Continue looking at this level for additional matches.
C
               NEXTR = 0
               PRVLVL= NLEVEL
               GOTO 10
C
C   The last keyword was an ellipsis, which means print everything from here
C   on down.
C
            ELSE
               IFOUND= IFOUND+1
               PRTSKP= .TRUE.
               CALL HLPPRL(LEVEL,PRVLVL,NLEVEL,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  NOMORE= .TRUE.
                  GOTO 20
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
                  GOTO 30
               ENDIF
               CALL HLPTXT(NLEVEL,ITEXT,NTEXT,IUNIT,ISTAT)
               IF (ISTAT.EQ.1) THEN
                  NOMORE= .TRUE.
                  GOTO 20
               ELSE IF (ISTAT.EQ.2) THEN
                  STATUS= 1
                  GOTO 30
               ELSE IF (ISTAT.EQ.3) THEN
                  STATUS= 2
                  GOTO 30
               ENDIF
               NEXTR = 1
               PRVLVL= NLEVEL+1
               GOTO 10
            ENDIF
C
C   No match here.  Keep looking.
C
         ELSE
            NEXTR = 0
            GOTO 10
         ENDIF
C
C   We're through the file.
C
   20 CONTINUE
C
C   Found a single entry.  Return NXTLVL pointing to the next level down only
C   if there is something more there.
C
      IF (IFOUND.EQ.1 .AND. .NOT.ELLIPS) THEN
         IF (NXTREC(NKEY).NE.0) THEN
            NXTLVL= NKEY+1
         ELSE
            NXTLVL= NKEY
         ENDIF
C
C   Either no entries found, or more than one.  Return one past the lowest
C   consecutive unique match (UNQLVL) for NXTLVL.
C
      ELSE
         NXTLVL= UNQLVL+1
C
C   If we didn't find what we wanted, print a list of what we could find for
C   sure, the "Sorry, no information on ..." message, and a list of additional
C   infomation at the next level down.
C
         IF (IFOUND.EQ.0 .AND. .NOT.NOMORE) THEN
            CALL HLPPRL(LEVEL,1,NXTLVL-1,ISTAT)
            IF (ISTAT.EQ.1) THEN
               GOTO 30
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
               GOTO 30
            ENDIF
            CALL HLPSRY(KEYWRD,NXTLVL,NKEY,ISTAT)
            IF (ISTAT.EQ.1) THEN
               GOTO 30
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
               GOTO 30
            ENDIF
            CALL HLPINF(UNQLVL,NXTREC(NXTLVL-1),IUNIT,ISTAT)
            IF (ISTAT.EQ.1) THEN
               GOTO 30
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 1
               GOTO 30
            ELSE IF (ISTAT.EQ.3) THEN
               STATUS= 2
               GOTO 30
            ENDIF
         ENDIF
      ENDIF
C
   30 CONTINUE
      RETURN
      END
      SUBROUTINE HLPINF(NLEVEL,ISREC,IUNIT,STATUS)
C
C   Print "additional information available" headings for level NLEVEL,
C   starting at record ISREC.
C
C      STATUS=1 - Cancel remaining HELP information.
C      STATUS=2 - Exit the HELP facility.
C      STATUS=3 - Error reading HELP file.
C
      INTEGER STATUS
C
      CHARACTER*80 TEXT,TEMP
      LOGICAL NEWLIN,QUALIF
C
      STATUS= 0
C
      IF (ISREC.EQ.0) GOTO 20
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
      TEMP  = ' '
      TEMP(2*NLEVEL+1:)= 'Additional information available:'
      CALL MORE(TEMP,1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
C
C   Arrange it such that all qualifiers are on separate lines from
C   nonqualifier entries.
C
      IREC  = ISREC
      TEMP  = ' '
      LTEMP = 2*NLEVEL-1
      NEWLIN= .TRUE.
      QUALIF= .FALSE.
   10    CONTINUE
         READ(IUNIT,REC=IREC,ERR=1010)
     C    TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
         CALL LTRIM(TEXT,LTEXT)
         IF (     (TEXT(1:1).EQ.'/' .AND. .NOT.QUALIF .AND. .NOT.NEWLIN)
     C       .OR. (TEXT(1:1).NE.'/' .AND.      QUALIF .AND. .NOT.NEWLIN)
     C       .OR. (LTEMP+LTEXT+1.GT.78))                            THEN
            CALL MORE(TEMP,1,ISTAT)
            IF (ISTAT.NE.0) THEN
               STATUS= ISTAT
               GOTO 20
            ENDIF
            LTEMP = 2*NLEVEL-1
            NEWLIN= .TRUE.
         ENDIF
         TEMP(LTEMP+2:)= TEXT
         LTEMP= ((LTEMP+LTEXT+1)/10)*10+9
         NEWLIN= .FALSE.
         IF (TEXT(1:1).EQ.'/') THEN
            QUALIF= .TRUE.
         ELSE
            QUALIF= .FALSE.
         ENDIF
C
C   More entries?
C
         IF (INEXT.NE.0) THEN
            IREC = INEXT
            GOTO 10
         ENDIF
C
C   Write out last line.
C
      IF (TEMP.NE.' ') THEN
         CALL MORE(TEMP,1,ISTAT)
         IF (ISTAT.NE.0) THEN
            STATUS= ISTAT
            GOTO 20
         ENDIF
      ENDIF
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
C
   20 CONTINUE
      RETURN
C
C   Error reading HELP file.
C
 1010 CONTINUE
      STATUS= 3
      GOTO 20
      END
      SUBROUTINE HLPPRL(LEVEL,ISLVL,IELVL,STATUS)
C
C   Print keyword levels ISLVL through IELVL.
C
C      STATUS=1 - Cancel remaining HELP information.
C      STATUS=2 - Exit the HELP facility.
C
      CHARACTER*(*) LEVEL(*)
      INTEGER STATUS
C
      CHARACTER*80 TEMP
C
      STATUS= 0
C
      IF (ISLVL.GT.IELVL) GOTO 20
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
      DO 10 I= ISLVL,IELVL
         TEMP  = ' '
         TEMP(2*I-1:)= LEVEL(I)
         CALL MORE(TEMP,1,ISTAT)
         IF (ISTAT.NE.0) THEN
            STATUS= ISTAT
            GOTO 20
         ENDIF
         CALL MORE(' ',1,ISTAT)
         IF (ISTAT.NE.0) THEN
            STATUS= ISTAT
            GOTO 20
         ENDIF
   10    CONTINUE
C
   20 CONTINUE
      RETURN
      END
      SUBROUTINE HLPSRY(KEYWRD,ISKEY,IEKEY,STATUS)
C
C   Write "Sorry, no information on ..." message.
C
C      STATUS=1 - Cancel remaining HELP information.
C      STATUS=2 - Exit the HELP facility.
C
      CHARACTER*(*) KEYWRD(*)
      INTEGER STATUS
C
      CHARACTER*80 KEY,TEMP
C
      STATUS= 0
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
      TEMP  = ' '
      TEMP(2*ISKEY-1:)= 'Sorry, no information on '
      LTEMP = 2*ISKEY-2+25
      DO 10 I= ISKEY,IEKEY
         KEY   = KEYWRD(I)
         CALL UPCASE(KEY)
         CALL PTRIM(KEY,LKEY)
         IF (LTEMP+LKEY+1.GT.78) THEN
            CALL MORE(TEMP,1,ISTAT)
            IF (ISTAT.NE.0) THEN
               STATUS= ISTAT
               GOTO 20
            ENDIF
            LTEMP = -1
         ENDIF
         TEMP(LTEMP+2:)= KEY
         LTEMP = LTEMP+LKEY+1
   10    CONTINUE
      IF (LTEMP.NE.-1) THEN
         CALL MORE(TEMP,1,ISTAT)
         IF (ISTAT.NE.0) THEN
            STATUS= ISTAT
            GOTO 20
         ENDIF
      ENDIF
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
C
   20 CONTINUE
      RETURN
      END
      SUBROUTINE HLPTOP(MKEY,NKEY,KEYWRD,LEVEL,NLEVEL,STATUS)
C
C   Prompt for additional help topic keywords.  NLEVEL indicates the level at
C   which to add keywords (and may be changed), and NKEY returns the final
C   number of keywords.
C
C      STATUS=1 - Exit the HELP facility.
C
      CHARACTER*(*) KEYWRD(MKEY),LEVEL(MKEY)
      INTEGER STATUS
C
      CHARACTER*80 KEY
      CHARACTER*132 TEMP
      CHARACTER*10 STR
C
      STATUS= 0
C
C   Check for too many keywords.
C
      IF (NLEVEL.GT.MKEY) THEN
         CALL INTCHA(MKEY,STR,LSTR)
         CALL WRNMSG(
     C 'Internal dimensions in HELP prevent accepting more than '
     C //STR(1:LSTR)//' keywords.',1)
         STATUS= 1
         GOTO 40
      ENDIF
C
C   If we ever go up past level 1, we should stop getting help.
C
   10 CONTINUE
      IF (NLEVEL.EQ.0) THEN
         STATUS= 1
         GOTO 40
      ENDIF
C
C   Collect the keywords.
C
      TEMP  = ' '
      LTEMP = -1
      DO 20 I= 1,NLEVEL-1
         KEY   = LEVEL(I)
         CALL LTRIM(KEY,LKEY)
         CALL UPCASE(KEY)
         IF (LTEMP+LKEY+1.LE.115) THEN
            TEMP(LTEMP+2:)= KEY
            LTEMP = LTEMP+LKEY+1
         ELSE
            TEMP(LTEMP+1:)= '...'
            LTEMP = LTEMP+3
         ENDIF
   20    CONTINUE
C
C   Add the "Subtopic?" prompt.
C
      IF (NLEVEL.EQ.1) THEN
         TEMP(LTEMP+2:)= 'Topic?'
         LTEMP = LTEMP+7
      ELSE
         TEMP(LTEMP+2:)= 'Subtopic?'
         LTEMP = LTEMP+10
      ENDIF
C
C   Prompt for new keywords.
C
      CALL GETCA(KEY,TEMP,1,' ',0,ISTAT)
      IF (ISTAT.EQ.1) THEN
         NLEVEL= NLEVEL-1
         GOTO 10
      ELSE IF (ISTAT.EQ.2) THEN
         STATUS= 1
         GOTO 40
      ENDIF
C
C   If the response is a "?", just return (to print help for the current set
C   of keywords again).
C
      IF (KEY.EQ.'?') THEN
         NKEY  = NLEVEL-1
         NLEVEL= MAX(NLEVEL-1,1)
C
C   Otherwise, load the rest of the keywords.
C
      ELSE
         NKEY  = NLEVEL
         KEYWRD(NKEY)= KEY
   30       CONTINUE
            CALL GETCA(KEY,' ',0,' ',0,ISTAT)
            IF (ISTAT.EQ.0) THEN
               IF (NKEY.EQ.MKEY) THEN
                  CALL INTCHA(MKEY,STR,LSTR)
                  CALL WRNMSG(
     C 'Internal dimensions in HELP prevent accepting more than '
     C //STR(1:LSTR)//' keywords.',1)
               ELSE
                  NKEY  = NKEY+1
                  KEYWRD(NKEY)= KEY
                  GOTO 30
               ENDIF
            ENDIF
      ENDIF
C
   40 CONTINUE
      RETURN
      END
      SUBROUTINE HLPTRV(IUNIT,IREC,TEXT,IUP,IDOWN,IPREV,INEXT,ITEXT,
     C                  NTEXT,NLEVEL,NEXT,PRTSKP,STATUS)
C
C   Traverse HELP Library tree.  For NEXT=-1, go up one level and to the next
C   entry; NEXT=0, find the next entry at the current level; NEXT=1, go down
C   one level.  IREC is the current record number.
C
C   If PRTSKP=TRUE, keep traversing the tree until we reach a point outside
C   the previous record's text (from record ITEXT to ITEXT+NTEXT-1).
C
C      STATUS=1 - No entry at the requested level.  For NEXT=-1, this means
C                 that we're at the top of the tree and STATUS will be set to
C                 2.  For NEXT=0, go up one level and to the next entry
C                 instead.  If THAT isn't there, set STATUS=2.  For NEXT=1, go
C                 to the next entry instead.  If that isn't there, go one
C                 level up and one over, and if THAT isn't there, (guess
C                 what?) set STATUS=2.
C      STATUS=2 - Off the end of the tree.  Point at record 1.
C      STATUS=3 - Error reading HELP file.
C
      CHARACTER*80 TEXT
      LOGICAL PRTSKP
      INTEGER STATUS
C
      STATUS= 0
      LNEXT = NEXT
      LSTEXT= ITEXT
      LETEXT= ITEXT+NTEXT-1
C
C   Go down one level.
C
   10 CONTINUE
      IF (LNEXT.GT.0) THEN
         IF (IDOWN.NE.0) THEN
            NLEVEL= NLEVEL+1
            IREC  = IDOWN
            READ(IUNIT,REC=IREC,ERR=1010)
     C       TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
C
C   Can't: go for the next entry at the current level.
C
         ELSE
            STATUS= 1
            LNEXT = 0
         ENDIF
      ENDIF
C
C   Go for the next current level entry.
C
      IF (LNEXT.EQ.0) THEN
         IF (INEXT.NE.0) THEN
            IREC  = INEXT
            READ(IUNIT,REC=IREC,ERR=1010)
     C       TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
C
C   Can't: go for the next level up and one over.
C
         ELSE
            STATUS= 1
            LNEXT = -1
         ENDIF
      ENDIF
C
C   Go for the next level up and one over.
C
      IF (LNEXT.LT.0) THEN
         IF (IUP.NE.0) THEN
            NLEVEL= NLEVEL-1
            IREC  = IUP
            READ(IUNIT,REC=IREC,ERR=1010)
     C       TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
            IF (INEXT.NE.0) THEN
               IREC  = INEXT
               READ(IUNIT,REC=IREC,ERR=1010)
     C          TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
C
C   Can't go over: go up again.
C
            ELSE
               STATUS= 1
               GOTO 10
            ENDIF
C
C   Can't: go for record 1.
C
         ELSE
            STATUS= 2
            IREC  = 1
            READ(IUNIT,REC=IREC,ERR=1010)
     C       TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
         ENDIF
      ENDIF
C
C   Check PRTSKP.
C
      IF (PRTSKP .AND. STATUS.NE.2) THEN
         IF (IREC.GE.LSTEXT .AND. IREC.LE.LETEXT) THEN
            IF (STATUS.EQ.1) LNEXT = LNEXT+1
            GOTO 10
         ENDIF
      ENDIF
C
   20 CONTINUE
      RETURN
C
C   Error reading HELP file.
C
 1010 CONTINUE
      STATUS= 3
      GOTO 20
      END
      SUBROUTINE HLPTXT(NLEVEL,ISREC,NREC,IUNIT,STATUS)
C
C   Write out the text for this help library entry, level NLEVEL, starting at
C   record ISREC and consisting of NREC records.
C
C      STATUS=1 - Cancel remaining HELP information.
C      STATUS=2 - Exit the HELP facility.
C      STATUS=3 - Error reading HELP file.
C
      CHARACTER*80 TEXT,TEMP
      INTEGER STATUS
C
      STATUS= 0
C
      INDENT= 2*NLEVEL+1
      DO 10 I= ISREC,ISREC+NREC-1
         READ(IUNIT,REC=I,ERR=1010)
     C    TEXT,N,IUP,IDOWN,IPREV,INEXT,ITEXT,NTEXT
         TEMP= ' '
         TEMP(INDENT:)= TEXT
         CALL MORE(TEMP,1,ISTAT)
         IF (ISTAT.NE.0) THEN
            STATUS= ISTAT
            GOTO 20
         ENDIF
   10    CONTINUE
      CALL MORE(' ',1,ISTAT)
      IF (ISTAT.NE.0) THEN
         STATUS= ISTAT
         GOTO 20
      ENDIF
C
   20 CONTINUE
      RETURN
C
C   Error reading HELP file.
C
 1010 CONTINUE
      STATUS= 3
      GOTO 20
      END
      SUBROUTINE IDXARG(NEXT,IVAL,STATUS)
C
C   Convert this or the next argument to an index value.  Valid 
C   keywords are "FIRST", "LAST", or "ALL".  We will translate "FIRST" into 
C   "1", "LAST" into "-1", and "ALL" into "0" right here.  Flag them with 
C   STATUS numbers, however, in case the calling routine wants to do anything 
C   special.  Note that integers less than 1 are not valid indices.
C
C      STATUS=1 - Error.
C      STATUS=2 - No more arguments.
C      STATUS=3 - "ALL"
C      STATUS=4 - "FIRST"
C      STATUS=5 - "LAST"
c
      parameter (maxdim=4)
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
      PARAMETER (NLIST=maxdim)
      CHARACTER*7 LIST(NLIST)
      INTEGER STATUS
      LOGICAL NEXT
C
C
      DATA LIST/'FIRST','LAST','ALL','-1'/
C
      STATUS= 0
C

C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXARG.GE.NARG) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXARG= IXARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at an '
     C    //'argument.',1)
         STOP 'IDXARG'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      CALL PTRIM(ARG(IXARG),LARG)
      CALL CVTINT(ARG(IXARG)(1:LARG),IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Not a valid integer.  Try "FIRST", "LAST", "ALL". 
C 
         CALL MATCH(.FALSE.,ARG(IXARG)(1:LARG),NLIST,LIST,ILIST,ISTAT)
         IF (ISTAT.EQ.0) THEN
C
C   It is one of these three.  Set the appropriate things.
C
            IF (ILIST.EQ.1) THEN
               IVAL  = 1
               STATUS= 4
            ELSE IF (ILIST.EQ.2) THEN
               IVAL  = -1
               STATUS= 5
            ELSE IF (ILIST.EQ.3) THEN
               IVAL  = 0
               STATUS= 3
            ENDIF
         ELSE IF (ISTAT.EQ.1) THEN
C
C   No matches.  This is an error.
C
            CALL ERRMSG('Invalid index '//ARG(IXARG)(1:LARG),1)
            STATUS= 1
C
C   No chance of multiple matches.
C
         ENDIF
c      ELSE IF (IVAL.LT.1) THEN
C
C   Integers less than 1 are not valid indices.
C
c         CALL ERRMSG('Integers less than 1 are not valid indices.',1)
c         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE IDXQAR(NEXT,IQ,IVAL,STATUS)
C
C   Convert this or the next qualifier argument to an index value.  Valid 
C   keywords are "FIRST", "LAST", or "ALL".  We will translate "FIRST" into 
C   "1", "LAST" into "-1", and "ALL" into "0" right here.  Flag them with 
C   STATUS numbers, however, in case the calling routine wants to do anything 
C   special.  Note that integers less than 1 are not valid indices.
C
C      STATUS=1 - Error.
C      STATUS=2 - No more arguments.
C      STATUS=3 - "ALL"
C      STATUS=4 - "FIRST"
C      STATUS=5 - "LAST"

      parameter (maxdim=4)
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      PARAMETER (NLIST=maxdim)
      CHARACTER*7 LIST(NLIST)
      INTEGER STATUS
      LOGICAL NEXT
C
C
C
C
      DATA LIST/'FIRST','LAST','ALL','-1'/
C
C
      STATUS= 0
C
C   Make sure we're pointing at a qualifier.
C
      IF (IQ.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier.',
     C               1)
         STOP 'IDXQAR'
      ENDIF
C
C   If this is a different qualifier, reset the argument pointer and the
C   qualifier pointer.
C
      IF (IQ.NE.IXQUAL) THEN
         IXQARG= 0
         IXQUAL= IQ
      ENDIF
      IF (NEXT) THEN
         IF (IXQARG.GE.NQARG(2,IQ)) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXQARG= IXQARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXQARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier'
     C    //' argument.',1)
         STOP 'IDXQAR'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      I= NQARG(1,IQ)+IXQARG-1
      CALL PTRIM(QARG(I),LQARG)
      CALL CVTINT(QARG(I)(1:LQARG),IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Not a valid integer.  Try "FIRST", "LAST", or "ALL".
C
         CALL MATCH(.FALSE.,QARG(I)(1:LQARG),NLIST,LIST,ILIST,ISTAT)
         IF (ISTAT.EQ.0) THEN
C
C   It is one of these three.  Set the appropriate things.
C
            IF (ILIST.EQ.1) THEN
               IVAL  = 1
               STATUS= 4
            ELSE IF (ILIST.EQ.2) THEN
               IVAL  = -1
               STATUS= 5
            ELSE IF (ILIST.EQ.3) THEN
               IVAL  = 0
               STATUS= 3
            ENDIF
         ELSE IF (ISTAT.EQ.1) THEN
C
C   No matches.  This is an error.
C
            CALL ERRMSG('Invalid index '//QARG(I)(1:LQARG),1)
            STATUS= 1
C
C   No chance of multiple matches.
C
         ENDIF
      ELSE IF (IVAL.LT.1) THEN
C
C   Integers less than 1 are not valid indices.
C
         CALL ERRMSG('Integers less than 1 are not valid indices.',1)
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE INITIO(JOUFIL,INIFIL)
C
C   Initialize standard I/O for the parser, set the mode and echo.
C   FORTRAN makes the first part easy, in that unit "*" is connected to the
C   standard I/O devices by default.  Flag the use of unit * by a zero for the
C   unit number.  
C
C   Open JOUFIL as a journal file to record all input.  Open and read INIFIL
C   (if it exists) as an auto-initialization command file.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
      EXTERNAL BPARIO
C     COMMON /MODE/ BATCH,ECHO,PRMPT
C     LOGICAL BATCH,ECHO,PRMPT
C
      CHARACTER*(*) JOUFIL,INIFIL
C
      LOGICAL LEXIST
      CHARACTER*80 TEMP
C
C   Zero the default unit numbers, clear the input and output file stacks.
C
      IOINP = 0
      IOOUT = 0
      NINSTK= 0
      NOUSTK= 0
C
C   Set the mode (batch or interactive) and command echoing.
C
      CALL SETMOD
      CALL SETECH
C
C   Open the journal file.  Signal no journal file with a zero unit number.
C
      IF (JOUFIL.NE.' ') THEN
         TEMP= JOUFIL
         CALL FILNAM(TEMP,'.JOU','OLD')
         CALL PTRIM(TEMP,LJOUFI)
         CALL DELFIL(TEMP(1:LJOUFI),.TRUE.,LEXIST)
         CALL GETLUN(IOJOU)
         OPEN(UNIT=IOJOU,FILE=TEMP(1:LJOUFI),STATUS='NEW',
     C        FORM='FORMATTED',ERR=1010)
      ELSE
         IOJOU= 0
      ENDIF
C
C   Read from the initialization file first, if it's there.
C
   10 CONTINUE
      IF (INIFIL.NE.' ') THEN
         TEMP= INIFIL
         CALL FILNAM(TEMP,'.COM','OLD')
         CALL PTRIM(TEMP,LINIFI)
         INQUIRE(FILE=TEMP(1:LINIFI),EXIST=LEXIST)
         IF (LEXIST) THEN
            CALL PSHINP(TEMP(1:LINIFI))
C           IF (.NOT.BATCH) THEN
C              ECHO  = .FALSE.
C              PRMPT = .FALSE.
C           ENDIF
C           CALL MSG('Reading commands from auto-initialization file '
C    C               //TEMP(1:LINIFI),1)
         ENDIF
      ENDIF
C
      RETURN
C
C   Error opening journal file.
C
 1010 CONTINUE
      CALL WRNMSG('Unable to start journal file '//TEMP(1:LJOUFI),1)
      CLOSE(UNIT=IOJOU)
      CALL FRELUN(IOJOU)
      IOJOU= 0
      GOTO 10
      END
      SUBROUTINE INTARG(NEXT,IVAL,STATUS)
C
C   Convert this or the next argument to an integer.
C
C      STATUS=1 - Conversion error.
C      STATUS=2 - No more arguments.
C
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
      INTEGER STATUS
      LOGICAL NEXT
C
      STATUS= 0
C
C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXARG.GE.NARG) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXARG= IXARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at an '
     C    //'argument.',1)
         STOP 'INTARG'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      CALL PTRIM(ARG(IXARG),LARG)
      CALL CVTINT(ARG(IXARG)(1:LARG),IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Error in conversion.
C
         CALL ERRMSG('Invalid integer '//ARG(IXARG)(1:LARG),1)
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE INTCHA(IVAL,STRING,LSTRIN)
C
C   Convert an integer into a character string with no leading blanks.
C
      CHARACTER*(*) STRING
C
      CHARACTER*20 TEMP,OVERFL
C
      DATA OVERFL/'********************'/
C
C   Write into a temporary space first.
C
      WRITE(TEMP,1) IVAL
    1 FORMAT(I20)
C
C   Trim off leading and trailing blanks, and copy the result into STRING.
C
      CALL LTRIM(TEMP,LTEMP)
      LSTRIN= LEN(STRING)
      IF (LTEMP.LE.LSTRIN) THEN
         STRING= TEMP
      ELSE
         STRING= OVERFL
      ENDIF
      LSTRIN= MIN(LTEMP,LSTRIN)
C
      RETURN
      END
      SUBROUTINE INTQAR(NEXT,IQ,IVAL,STATUS)
C
C   Convert this or the next qualifier argument to an integer.
C
C      STATUS=1 - Conversion error.
C      STATUS=2 - No more arguments.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      INTEGER STATUS
      LOGICAL NEXT
C
      STATUS= 0
C
C   Make sure we're pointing at a qualifier.
C
      IF (IQ.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier.',
     C               1)
         STOP 'INTQAR'
      ENDIF
C
C   If this is a different qualifier, reset the argument pointer and the
C   qualifier pointer.
C
      IF (IQ.NE.IXQUAL) THEN
         IXQARG= 0
         IXQUAL= IQ
      ENDIF
      IF (NEXT) THEN
         IF (IXQARG.GE.NQARG(2,IQ)) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXQARG= IXQARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXQARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier'
     C    //' argument.',1)
         STOP 'INTQAR'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      I= NQARG(1,IQ)+IXQARG-1
      CALL PTRIM(QARG(I),LQARG)
      CALL CVTINT(QARG(I)(1:LQARG),IVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Error in conversion.
C
         CALL ERRMSG('Invalid integer '//QARG(I)(1:LQARG),1)
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE LOCASE(STRING)
C
C   Convert this character string to lower case.
C
      CHARACTER*(*) STRING
      CHARACTER*26 LOWER,UPPER
C
      DATA LOWER/'abcdefghijklmnopqrstuvwxyz'/
     C     UPPER/'ABCDEFGHIJKLMNOPQRSTUVWXYZ'/
C
C   Don't worry about the trailing blanks -
C
      CALL PTRIM(STRING,LSTRIN)
C
C   Look for upper case letters and substitute lower case ones.
C
      DO 10 I= 1,LSTRIN
         LETTER= INDEX(UPPER,STRING(I:I))
         IF (LETTER.NE.0) STRING(I:I)= LOWER(LETTER:LETTER)
   10    CONTINUE
      RETURN
      END
      SUBROUTINE LTRIM(STRING,LSTRIN)
C
C   Remove leading blanks, nulls, or tabs from STRING, then return the length 
C   of STRING after trailing blanks and nulls have been removed.
C
      CHARACTER*(*) STRING
C
      CHARACTER NULL,TAB
C
C   Initialize null and tab characters.
C
      NULL= CHAR(0)
      TAB = CHAR(9)
C
      LSTRIN= LEN(STRING)
      IS= 1
      DO 10 I= 1,LSTRIN
         IF (STRING(I:I).NE.' ' .AND. STRING(I:I).NE.NULL
     C                          .AND. STRING(I:I).NE.TAB) THEN
            IS= I
            GOTO 20
         ENDIF
   10    CONTINUE
C
   20 CONTINUE
      CALL CCOPY(STRING,IS,LSTRIN-IS+1,1,LSTRIN)
      CALL PTRIM(STRING,LSTRIN)
      RETURN
      END
      SUBROUTINE MATCH(EXACT,STRING,NLIST,LIST,ILIST,STATUS)
C
C   Try to match STRING with elements in LIST.  To avoid confusion, everything
C   is converted to upper case.  If EXACT=TRUE, only accept exact matches.
C
C      STATUS=1 - No matches.
C      STATUS=2 - Multiple matches.
C
      LOGICAL EXACT
      CHARACTER*(*) STRING,LIST(*)
      INTEGER STATUS
C
      STATUS= 0
C
C   We need to fix up the incoming string a bit.  If it's all blanks, give us
C   something to compare with, so we don't match everything.
C
      CALL UPCASE(STRING)
      CALL PTRIM(STRING,LSTRIN)
      IF (LSTRIN.EQ.0) LSTRIN= 1
C
C   Look through the list.
C
      ILIST= 0
      NMATCH= 0
      DO 10 I= 1,NLIST
C
C   Also get the thing we're comparing with in upper case.
C
         CALL UPCASE(LIST(I))
C
C   If we match the string exactly, this is it, no question.
C
         IF (LIST(I).EQ.STRING(1:LSTRIN)) THEN
            ILIST= I
            GOTO 20
C
C   Otherwise, check to see if we match the first LSTRIN characters.  We now
C   have to check for uniqueness.
C
         ELSE IF (.NOT.EXACT) THEN
            IF (INDEX(LIST(I),STRING(1:LSTRIN)).EQ.1) THEN
               ILIST= I
               NMATCH= NMATCH+1
            ENDIF
         ENDIF
   10    CONTINUE
C
C   Check the number of matches.
C
      IF (NMATCH.EQ.0) THEN
         STATUS= 1
      ELSE IF (NMATCH.GT.1) THEN
         STATUS= 2
      ENDIF
C
   20 CONTINUE
      RETURN
      END
      SUBROUTINE MORE(TEXT,N,STATUS)
C
C   Write text to the current output device, with pauses at 22-line intervals
C   if the input device is interactive.  Use MSG to put the text out.  If
C   N=0, reinitialize line counting.
C
C      STATUS=1 - Something other than <RETURN> was typed in response to the
C                 prompt.
C      STATUS=2 - An end-of-file was encountered in response to the prompt.
C
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
C
      CHARACTER*(*) TEXT(*)
      INTEGER STATUS
C
      PARAMETER (NPRMST=2)
      CHARACTER*30 PRMSTR(NPRMST)
      CHARACTER*80 LINE
C
      DATA MLINES/22/,NLINES/0/
      DATA PRMSTR/' ','Type <RETURN> to continue...'/
C
      STATUS= 0
C
C   If we're not currently interactive, just dump the stuff.
C
      IF (ECHO) THEN
         CALL MSG(TEXT,N)
C
C   Otherwise...
C
      ELSE
         ISTART= 1
         NLEFT = N
   10    CONTINUE
C
C   If N=0, reinitialize our line counting.
C
         IF (N.EQ.0) THEN
            NLINES= 0
C
C   If this text doesn't put us over, write it all out.
C
         ELSE IF (NLEFT+NLINES.LE.MLINES) THEN
            CALL MSG(TEXT(ISTART),NLEFT)
            NLINES= NLINES+NLEFT
C
C   Otherwise print some of it out and prompt.
C
         ELSE IF (NLEFT+NLINES.GT.MLINES) THEN
            CALL MSG(TEXT(ISTART),MLINES-NLINES)
            CALL PROMPT(PRMSTR,NPRMST)
C
C   In response to the prompt, we expect a <RETURN>.  If we get some text,
C   return STATUS=1 and parse the text into arguments.  If we get an
C   end-of-file, return STATUS=2.
C
            CALL RDLIN(LINE,ISTAT)
            IF (ISTAT.EQ.1) THEN
               ISTART= ISTART+(MLINES-NLINES)
               NLEFT = NLEFT -(MLINES-NLINES)
               NLINES= 0
               GOTO 10
            ELSE IF (ISTAT.EQ.0) THEN
               STATUS= 1
               CALL FNDARG(LINE,ISTAT)
            ELSE IF (ISTAT.EQ.2) THEN
               STATUS= 2
            ENDIF
         ENDIF
C
      ENDIF
      RETURN
      END
      SUBROUTINE MSG(TEXT,NLINES)
C
C   Write some sort of message to the current output device.
C
      CHARACTER*(*) TEXT(*)
C
      CHARACTER*132 TEMP
C
C   Trim it and write it out.
C
      DO 10 I= 1,NLINES
         TEMP= '  '//TEXT(I)
         CALL PTRIM(TEMP,LTEMP)
         CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
   10    CONTINUE
      RETURN
      END
      SUBROUTINE POPINP(STATUS)
C
C   Pop a filename off the input file stack and set echo.  Set the unit number
C   IOINP, and close and free the old unit number.  If there are no more
C   entries in the stack, use the standard input device.
C
C      STATUS=1 - Input stack is already empty.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
C
      INTEGER STATUS
C
      STATUS= 0
C
C   Always check for a zero unit number.  This flags the use of the standard
C   input device, and as such should not (or need not) be closed.
C
      IF (NINSTK.GT.0) THEN
         IF (INPSTK(NINSTK).NE.0) THEN
            CLOSE(UNIT=INPSTK(NINSTK))
            CALL FRELUN(INPSTK(NINSTK))
         ENDIF
C
C   Pop the stack, if there's something there.
C
         IF (NINSTK.GT.1) THEN
            NINSTK= NINSTK-1
            IOINP = INPSTK(NINSTK)
         ELSE
            NINSTK= 0
            IOINP = 0
         ENDIF
         CALL SETECH
C
C   Stack was already empty.
C
      ELSE
         STATUS= 1
      ENDIF
C
      RETURN
      END
      SUBROUTINE POPOUT(STATUS)
C
C   Pop a filename off the output file stack.  Set the unit number
C   IOOUT, and close and free the old unit number.  If there are no more
C   entries in the stack, use the standard output device.
C
C      STATUS=1 - Output stack is already empty.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
C
      INTEGER STATUS
C
      STATUS= 0
C
C   Always check for a zero unit number.  This flags the use of the standard
C   output device, and as such should not (or need not) be closed.
C
      IF (NOUSTK.GT.0) THEN
         IF (OUTSTK(NOUSTK).NE.0) THEN
            CLOSE(UNIT=OUTSTK(NOUSTK))
            CALL FRELUN(OUTSTK(NOUSTK))
         ENDIF
C
C   Pop the stack, if there's something there.
C
         IF (NOUSTK.GT.1) THEN
            NOUSTK= NOUSTK-1
            IOOUT = OUTSTK(NOUSTK)
         ELSE
            NOUSTK= 0
            IOOUT = 0
         ENDIF
         CALL SETECH
C
C   Stack was already empty.
C
      ELSE
         STATUS= 1
      ENDIF
C
      RETURN
      END
      SUBROUTINE POPPRM(STATUS)
C
C   Pop the previous prompt string off the stack.
C
C      STATUS=1 - Prompt string stack is already empty.
C
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
      PARAMETER (MSTACK=20)
      COMMON /PARPST/ NPRSTK,LPRSTK(MSTACK)
      COMMON /PARPSC/ PRMSTK(MSTACK)
      CHARACTER*20 PRMSTK
C
      INTEGER STATUS
C
      STATUS= 0
C
      IF (NPRSTK.GT.0) THEN
         IF (NPRSTK.GT.1) THEN
            NPRSTK= NPRSTK-1
            LPRMCH= LPRSTK(NPRSTK)
            PRMCHA= PRMSTK(NPRSTK)
         ELSE
            NPRSTK= 0
            LPRMCH= 1
            PRMCHA= '?'
         ENDIF
C
      ELSE
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE PROMPT(TEXT,NLINES)
C
C   Prompt for input from the current input device.  Don't do a carriage 
C   return after the text.  Signal this by using a "$" as carriage control.  A
C   zero for NLINES implies put out the prompt string instead.
C
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
      EXTERNAL BPARPR
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
C
C
      CHARACTER*(*) TEXT(*)
C
      CHARACTER*132 TEMP
C
C   If PRMPT is FALSE, don't do anything.
C
      IF (PRMPT) THEN
C
C   If NLINES is zero, put out the prompt string instead.
C
         IF (NLINES.EQ.0) THEN
            TEMP= '$'//PRMCHA(1:LPRMCH)
            CALL WRLIN(TEMP(1:LPRMCH+1))
C
         ELSE
            DO 10 I= 1,NLINES-1
               TEMP= '  '//TEXT(I)
               CALL PTRIM(TEMP,LTEMP)
               CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
   10          CONTINUE
C
C   Leave the cursor at the end of the last line.  (If the last line isn't 
C   blank, give it an extra two spaces.)
C
            CALL PTRIM(TEXT(NLINES),LTEXT)
            IF (LTEXT.EQ.0) THEN
               CALL WRLIN('$ ')
            ELSE
               TEMP= '$ '//TEXT(NLINES)//'  '
               CALL PTRIM(TEMP,LTEMP)
               CALL WRLIN(TEMP(1:MAX(MIN(LTEMP+2,LEN(TEMP)),1)))
            ENDIF
         ENDIF
C
      ENDIF
      RETURN
      END
      SUBROUTINE PSHINP(FILE)
C
C   Push this filename onto the input file stack and open it, connected with a
C   new unit number put in IOINP.  Set echo.  Leave the old file open.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
      EXTERNAL BPARIO
C
      CHARACTER*(*) FILE
C
      CHARACTER*80 STDDEV,TEMP
C
      IF (NINSTK.EQ.MSTACK) THEN
         CALL ERRMSG('Input file stack is full.',1)
      ELSE
C
C   If it's the standard input device, don't open it, just use the flag "0"
C   for the unit number.
C
         IF (FILE.EQ.STDDEV('INPUT')) THEN
            INPSTK(NINSTK+1)= 0
C
C   An input file might belong to another user: open the file READONLY if we
C   can.
C
         ELSE
            TEMP= FILE
            CALL FILNAM(TEMP,'.COM','OLD')
            CALL PTRIM(TEMP,LTEMP)
            CALL GETLUN(INPSTK(NINSTK+1))
            OPEN(UNIT=INPSTK(NINSTK+1),FILE=TEMP(1:LTEMP),STATUS='OLD',
     C           FORM='FORMATTED',ERR=1010)
         ENDIF
C
         NINSTK= NINSTK+1
         IOINP = INPSTK(NINSTK)
         CALL SETECH
      ENDIF
C
   20 CONTINUE
      RETURN
C
C   Error on open.
C
 1010 CONTINUE
      CALL FRELUN(INPSTK(NINSTK+1))
      CALL ERRMSG('Unable to open input file '//TEMP(1:LTEMP),1)
      GOTO 20
      END
      SUBROUTINE PSHOUT(FILE)
C
C   Push this filename onto the output file stack and open it, connected with a
C   new unit number put in IOOUT.  Leave the old file open.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
      EXTERNAL BPARIO
C
      CHARACTER*(*) FILE
C
      CHARACTER*80 STDDEV,TEMP
      LOGICAL LEXIST
C
      IF (NOUSTK.EQ.MSTACK) THEN
         CALL ERRMSG('Output file stack is full.',1)
      ELSE
C
C   If it's the standard output device, don't open it, just use the flag "0"
C   for the unit number.
C
         IF (FILE.EQ.STDDEV('OUTPUT')) THEN
            OUTSTK(NOUSTK+1)= 0
C
C   Use the .LOG file type if the filename doesn't have a type yet.  (Using
C   the "OLD" flag when calling FILNAM ensures that an existing file type is
C   left alone.)
C
         ELSE
            TEMP= FILE
            CALL FILNAM(TEMP,'.LOG','OLD')
            CALL PTRIM(TEMP,LTEMP)
            CALL DELFIL(TEMP(1:LTEMP),.TRUE.,LEXIST)
            CALL GETLUN(OUTSTK(NOUSTK+1))
            OPEN(UNIT=OUTSTK(NOUSTK+1),FILE=TEMP(1:LTEMP),STATUS='NEW',
     C           FORM='FORMATTED',ERR=1010)
         ENDIF
C
         NOUSTK= NOUSTK+1
         IOOUT = OUTSTK(NOUSTK)
         CALL SETECH
      ENDIF
C
   20 CONTINUE
      RETURN
C
C   Error on open.
C
 1010 CONTINUE
      CALL FRELUN(OUTSTK(NOUSTK+1))
      CALL ERRMSG('Unable to open output file '//TEMP(1:LTEMP),1)
      GOTO 20
      END
      SUBROUTINE PSHPRM(PROMPT)
C
C   Push a prompt string onto the parser prompt string stack.
C
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
      PARAMETER (MSTACK=20)
      COMMON /PARPST/ NPRSTK,LPRSTK(MSTACK)
      COMMON /PARPSC/ PRMSTK(MSTACK)
      CHARACTER*20 PRMSTK
      EXTERNAL BPARPR
C
      CHARACTER*(*) PROMPT
C
      IF (NPRSTK.EQ.MSTACK) THEN
         CALL ERRMSG('Prompt string stack is full.',1)
C
C   Move the prompt string into the common array.
C
      ELSE
         LPROMP= LEN(PROMPT)
         LPRSTK(NPRSTK+1)= MIN(LPROMP,LEN(PRMSTK(NPRSTK+1)))
         PRMSTK(NPRSTK+1)= PROMPT
C
C   Check the length of the prompt character string.
C
         IF (LPROMP.GT.LEN(PRMSTK(NPRSTK+1))) THEN
            CALL WRNMSG('Prompt string truncated to '
     C                  //PRMSTK(NPRSTK+1),1)
         ENDIF
         NPRSTK= NPRSTK+1
         LPRMCH= LPRSTK(NPRSTK)
         PRMCHA= PRMSTK(NPRSTK)
C
      ENDIF
      RETURN
      END
      SUBROUTINE QUALIF(NQSET,NQLIST,QLIST,IALIST,IQLIST,STATUS)
C
C   Compare this list of valid qualifiers with the ones we found on the command
C   line.  
C
C   NQSET is the number of SETS of qualifiers.  (An example of one set would be
C   "/FORMATTED" and "/UNFORMATTED".  These should not both be given on the 
C   same command line.)  NQLIST is an array giving the number of valid 
C   qualifiers in each set, and QLIST is a list of all possible qualifiers, of
C   which there are Summation (from i=1 to NQSET) of NQLIST(i).  Right?
C
C   There is an IALIST element for every QLIST element.  IALIST gives the 
C   number of arguments that are expected with this qualifier (as in 
C   "/LOCATION=(.8,.5)" expecting two arguments).  IF IALIST WAS NOT ZERO
C   coming into QUALIF, the index of the qualifier in QUAL (the qualifiers 
C   found on the command line) is returned in IALIST.  This allows later 
C   routines (such as xxxQAR) to access the qualifier arguments, either picking
C   up arguments or reading new ones in.  If IALIST is negative, we expect
C   arguments, but we're not sure how many.
C
C   IQLIST returns the index IN THIS SET of the qualifier that was found, i.e.
C   if the qualifier "/UNFORMATTED" was given, IQLIST(i) would be set to 2
C   (corresponding to the example above).  If no qualifier in this particular 
C   set was given, IQLIST(i) will return a zero.
C
C   Note that the case of NO VALID QUALIFIERS is signalled by setting NQSET=1
C   and NQLIST(1)=0.
C
C   See subroutine FNDQUA for a description of NQARG and QARG.
C
C      STATUS=1 - Error in qualifiers.
C
      PARAMETER (MQUAL=20)
      COMMON /PARQUA/ NQUAL
      COMMON /PARQUC/ QUAL(MQUAL)
      CHARACTER*80 QUAL
      PARAMETER (MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
C
      CHARACTER*(*)  QLIST(*)
      DIMENSION     IALIST(*)
      DIMENSION NQLIST(NQSET),IQLIST(NQSET)
      INTEGER STATUS
C
      PARAMETER (MQENT=200)
      DIMENSION IQINDX(2,MQENT)
      CHARACTER*80 TEMP(3)
      CHARACTER*10 STR,STR1
C
      STATUS= 0
C
C   First let's check for the case of no valid qualifiers.
C
      IF (NQLIST(1).EQ.0) THEN
         IF (NQUAL.NE.0) THEN
            CALL ERRMSG('No qualifiers allowed.',1)
            STATUS= 1
         ENDIF
         GOTO 110
      ENDIF
C
C   Come up with the total number of qualifiers in QLIST, and a way to index
C   back to put the pointer in IQLIST.  Also, zero IQLIST.
C
C   Check total number of qualifiers first.
C
      NLIST= 0
      DO 5 IQS= 1,NQSET
         NLIST= NLIST+NQLIST(IQS)
    5    CONTINUE
      IF (NLIST.GT.MQENT) THEN
         CALL INTCHA(NLIST,STR,LSTR)
         CALL INTCHA(MQENT,STR1,LSTR1)
         WRITE(TEMP,6) STR(1:LSTR),STR1(1:LSTR1)
    6    FORMAT('Programming error.  This command passes too ',
     C          'many qualifiers to'/'routine QUALIF.  ',
     C          A,' were passed; internal dimension is ',A)
         CALL ERRMSG(TEMP,2)
         STOP 'QUALIF'
      ENDIF
C
      NLIST= 0
      DO 10 IQS= 1,NQSET
         IQLIST(IQS)= 0
         DO 10 IQL= 1,NQLIST(IQS)
            NLIST= NLIST+1
            IQINDX(1,NLIST)= IQS
            IQINDX(2,NLIST)= IQL
   10       CONTINUE
C
C   Now, given ILIST, a pointer into the one-dimensional array QLIST returned
C   from MATCH, IQINDX(1,ILIST) tells what set the qualifier is in, and
C   IQINDX(2,ILIST) is the index into that set, which is the number to be
C   returned in IQLIST.
C
      DO 100 IQ= 1,NQUAL
C
C   Try to match this qualifier with the list of valid qualifiers.
C
         CALL MATCH(.FALSE.,QUAL(IQ),NLIST,QLIST,ILIST,ISTAT)
         IF (ISTAT.EQ.1) THEN
            CALL PTRIM(QUAL(IQ),LQUAL)
            WRITE(TEMP,11) QUAL(IQ)(1:LQUAL)
   11       FORMAT('Qualifier /',A,' not recognized.')
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ELSE IF (ISTAT.EQ.2) THEN
            CALL PTRIM(QUAL(IQ),LQUAL)
            WRITE(TEMP,12) QUAL(IQ)(1:LQUAL)
   12       FORMAT('Ambiguous qualifier /',A)
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ENDIF
C
C   Check to see if this IQLIST element has been set before, and if not, set
C   it.
C
         IQS= IQINDX(1,ILIST)
         IQL= IQINDX(2,ILIST)
         IF (IQLIST(IQS).NE.0) THEN
            ILIST2= ILIST+IQLIST(IQS)-IQL
            CALL PTRIM(QLIST(ILIST),LLIST)
            CALL PTRIM(QLIST(ILIST2),LLIST2)
            WRITE(TEMP,13) QLIST(ILIST)(1:LLIST),QLIST(ILIST2)(1:LLIST2)
   13       FORMAT('Incompatible qualifier combination /',A,' and /',A)
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ENDIF
         IQLIST(IQS)= IQL
C
C   Check for arguments when there shouldn't be.
C
         IF (NQARG(2,IQ).NE.0 .AND. IALIST(ILIST).EQ.0) THEN
            CALL PTRIM(QLIST(ILIST),LLIST)
            WRITE(TEMP,14) QLIST(ILIST)(1:LLIST)
   14       FORMAT('Arguments are not allowed with qualifier /',A)
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ENDIF
C
C   Check for too many arguments (if IALIST > 0).
C
         IF (IALIST(ILIST).GT.0 .AND. NQARG(2,IQ).GT.IALIST(ILIST)) THEN
            CALL PTRIM(QLIST(ILIST),LLIST)
            WRITE(TEMP,15) QLIST(ILIST)(1:LLIST)
   15       FORMAT('Too many arguments for qualifier /',A)
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ENDIF
C
C   Check for not enough arguments (if IALIST > 0).
C
         IF (IALIST(ILIST).GT.0 .AND. NQARG(2,IQ).LT.IALIST(ILIST)
     C                          .AND. NQARG(2,IQ).NE.0)             THEN
            CALL PTRIM(QLIST(ILIST),LLIST)
            WRITE(TEMP,16) QLIST(ILIST)(1:LLIST)
   16       FORMAT('Not enough arguments for qualifier /',A)
            CALL ERRMSG(TEMP,1)
            STATUS= 1
            GOTO 110
         ENDIF
C
C   Finally, if there were arguments, pass the pointer back.
C
         IF (IALIST(ILIST).NE.0) IALIST(ILIST)= IQ
  100    CONTINUE
C
  110 CONTINUE
      RETURN
      END
      SUBROUTINE RDARG(STATUS)
C
C   Read a line of input and parse it into arguments.
C
C      STATUS=1 - Error.
C      STATUS=2 - Got just a carriage return.
C      STATUS=3 - End-of-file entered.
C
      INTEGER STATUS
      CHARACTER*255 LINE
C
      STATUS= 0
C
C   Read a line of input.
C
      CALL RDLIN(LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         STATUS= 2
         GOTO 10
      ELSE IF (ISTAT.EQ.2) THEN
         STATUS= 3
         GOTO 10
      ENDIF
C
C   Parse it into arguments.
C
      CALL FNDARG(LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         STATUS= 1
         GOTO 10
      ENDIF
C
   10 CONTINUE
      RETURN
      END
      SUBROUTINE RDLIN(LINE,STATUS)
C
C   Read a line of input from the current input device into LINE.  If ECHO is
C   TRUE, echo the line to the current output device.  Anything after a "!" is
C   considered a comment, and is blanked out after the line has been echoed.
C   A three letter line "EOF" is recognized as the equivalent of an end-of-file
C   from the READ statement (^Z in VAX/VMS).
C
C      STATUS=1 - Zero length input line.
C      STATUS=2 - End-of-file on read.
C
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
C
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      PARAMETER (NCOLS=255)
      CHARACTER*255 TEMP
      CHARACTER*3 EOF
      CHARACTER NULL,TAB
      LOGICAL QUOTE,SUBQUO
C
      STATUS= 0
      NULL  = CHAR(0)
      TAB   = CHAR(9)
      LLINE = LEN(LINE)
C
C   Initialize LINE to all blanks.
C
   10 CONTINUE
      LINE  = ' '
      IE    = 1
      ICOMM = IE+1
C
C   Read at least some of the line.  Cover for the case when the input is 
C   longer than LINE--it may not be trapped as a read error.
C
C   Read up to NCOLS characters per READ.
C
  100    CONTINUE
         IF (IOINP.EQ.0) THEN
            READ(    *,101,ERR=1010,END=1020) 
     C                                   LINE(IE:MIN(IE-1+NCOLS,LLINE))
         ELSE
            READ(IOINP,101,ERR=1010,END=1020) 
     C                                   LINE(IE:MIN(IE-1+NCOLS,LLINE))
         ENDIF
  101    FORMAT(A)
         CALL PTRIM(LINE(IE:),INLEN)
         IF (IE+INLEN-1.GT.LLINE) GOTO 1010
         IF (ECHO) THEN
            IF (INLEN.EQ.0) THEN
               TEMP  = ' '
               LTEMP = 1
            ELSE
               TEMP  = ' '//LINE(IE:MIN(IE+INLEN-1,LLINE))
               LTEMP = 1+(MIN(IE+INLEN-1,LLINE)-IE+1)
            ENDIF
            CALL WRLIN(TEMP(1:LTEMP))
         ENDIF
C
         IE    = IE+INLEN-1
         ICOMM = IE+1

C
C   Zero length line?
C
         IF (IE.EQ.0) THEN
            STATUS= 1
            GOTO 300
         ENDIF
C
C   Check for a '-' at the end, indicating that the line is to be continued.
C
         IF (LINE(IE:IE).EQ.'-' .AND. INLEN.NE.0) THEN
            LINE(IE:IE)= ' '
            CALL WRLIN('$_')
            GOTO 100
         ENDIF
C
C   Done reading the line.
C
  200 CONTINUE
C
C   Expand out symbols.
C
      CALL TRNLIN(LINE,ISTAT)
      IF (ISTAT.NE.0) THEN
         CALL PROMPT('Reenter line:',1)
         GOTO 10
      ENDIF
C
C   Look for an exclamation point, not within quotes.  If we find one, we will
C   blank it, and everything that follows it, out before returning.
C
      QUOTE = .FALSE.
      SUBQUO= .FALSE.
      ICOMM = IE+1
      DO 210 I= 1,IE
         IF (SUBQUO) THEN
            IF (LINE(I:I).EQ.'"') SUBQUO= .FALSE.
         ELSE IF (QUOTE) THEN
            IF (LINE(I:I).EQ.'"') THEN
               IF (I.EQ.IE) THEN
                  QUOTE = .FALSE.
               ELSE IF (LINE(I+1:I+1).EQ.' ' .OR. LINE(I+1:I+1).EQ.',')
     C          THEN
                  QUOTE = .FALSE.
               ELSE
                  SUBQUO= .TRUE.
               ENDIF
            ENDIF
         ELSE IF (LINE(I:I).EQ.'"') THEN
            IF (I.EQ.1) THEN
               QUOTE = .TRUE.
            ELSE IF (LINE(I-1:I-1).EQ.' ' .OR. LINE(I-1:I-1).EQ.',')
     C       THEN
               QUOTE = .TRUE.
            ELSE
               SUBQUO= .TRUE.
            ENDIF
         ELSE IF (LINE(I:I).EQ.'!') THEN
            ICOMM = I
            GOTO 220
         ENDIF
  210    CONTINUE
C
C   Find the first nonblank character, and check for "@" or "EOF".
C
  220 CONTINUE
      DO 230 I= 1,IE
         IF (LINE(I:I).NE.' ' .AND. LINE(I:I).NE.NULL
     C                        .AND. LINE(I:I).NE.TAB) GOTO 240
  230    CONTINUE
      I= IE
C
  240 CONTINUE
C
C   If the first character is a @, what follows is supposed to be a filename 
C   for input.  Push this file onto the input file stack and open it.  Go back
C   and read the line we were SUPPOSED to get.
C
      IF (LINE(I:I).EQ.'@') THEN
         CALL PSHINP(LINE(I+1:ICOMM-1))
         CALL PROMPT('New line:',1)
         GOTO 10
C
C   If we've got an EOF (and ONLY an EOF), pop the input file stack and read 
C   the next line.  If we're already at the top of the stack, signal an 
C   end-of-file.
C
      ELSE
         IF (ICOMM.GT.I) THEN
            CALL PTRIM(LINE(I:ICOMM-1),LEOF)
            IF (LEOF.EQ.3) THEN
               EOF= LINE(I:I+LEOF-1)
               CALL UPCASE(EOF)
               IF (EOF.EQ.'EOF') THEN
                  CALL POPINP(ISTAT)
                  IF (ISTAT.EQ.0) THEN
                     CALL PROMPT('New line:',1)
                     GOTO 10
                  ENDIF
                  STATUS= 2
               ENDIF
            ENDIF
         ENDIF
      ENDIF
C
  300 CONTINUE
C
C   The line is valid input.  Echo it to the journal file.
C
      IF (STATUS.NE.2) THEN
         IF (IOJOU.NE.0) WRITE(IOJOU,101) LINE(1:MAX(IE,1))
C
C   Blank out the comment in the line.
C
         IF (ICOMM.LE.IE) LINE(ICOMM:IE)= ' '
      ENDIF
      RETURN
C
C   Error on input.
C
 1010 CONTINUE
      CALL PTRIM(LINE(IE:),INLEN)
      IF (ECHO) THEN
         IF (INLEN.EQ.0) THEN
            TEMP  = ' '
            LTEMP = 1
         ELSE
            TEMP  = ' '//LINE(IE:MIN(IE+INLEN-1,LLINE))
            LTEMP = 1+(MIN(IE+INLEN-1,LLINE)-IE+1)
         ENDIF
         CALL WRLIN(TEMP(1:LTEMP))
      ENDIF
      CALL ERRMSG('Input line was too long.',1)
      CALL PROMPT('Reenter line:',1)
      GOTO 10
C
C   End-of-file through the READ statement.  Pop the input stack and get 
C   another line.  If the input stack is empty, flag an end-of-file and return.
C
 1020 CONTINUE
      IF (ECHO) CALL WRLIN(' EOF')
      CALL POPINP(ISTAT)
      IF (ISTAT.EQ.0) THEN
         CALL PROMPT('New line:',1)
         GOTO 10
      ENDIF
      STATUS= 2
      GOTO 300
      END
      SUBROUTINE RDQAR(IQ,STATUS)
C
C   Read a line of input and parse it into qualifier arguments for qualifier
C   IQ.
C
C      STATUS=1 - Error.
C      STATUS=2 - Got just a carriage return.
C      STATUS=3 - End-of-file entered.
C
      INTEGER STATUS
      CHARACTER*255 LINE
C
      STATUS= 0
C
C   Read a line of input.
C
      CALL RDLIN(LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         STATUS= 2
         GOTO 10
      ELSE IF (ISTAT.EQ.2) THEN
         STATUS= 3
         GOTO 10
      ENDIF
C
C   Parse it into arguments.
C
      CALL FNDQAR(IQ,LINE,ISTAT)
      IF (ISTAT.EQ.1) THEN
         STATUS= 1
         GOTO 10
      ENDIF
C
   10 CONTINUE
      RETURN
      END
      SUBROUTINE REAARG(NEXT,RVAL,STATUS)
C
C   Convert this or the next argument to a real number.
C
C      STATUS=1 - Conversion error.
C      STATUS=2 - No more arguments.
C
      PARAMETER (MARG=20)
      COMMON /PARARG/ NARG,IXARG
      COMMON /PARARC/ ARG(MARG)
      CHARACTER*80 ARG
      INTEGER STATUS
      LOGICAL NEXT
C
      STATUS= 0
C
C   If NEXT, check that we're not going off the end of the list, then advance
C   the pointer.
C
      IF (NEXT) THEN
         IF (IXARG.GE.NARG) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXARG= IXARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at an '
     C    //'argument.',1)
         STOP 'REAARG'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      CALL PTRIM(ARG(IXARG),LARG)
      CALL CVTREA(ARG(IXARG)(1:LARG),RVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Error in conversion.
C
         CALL ERRMSG('Invalid real number '//ARG(IXARG)(1:LARG),1)
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE REACHA(RVAL,STRING,LSTRIN)
C
C   Convert a real number into a character string with no leading blanks.
C
      CHARACTER*(*) STRING
C
      CHARACTER*20 TEMP,OVERFL
C
      DATA OVERFL/'********************'/
C
C   Write into a temporary space first.
C
      WRITE(TEMP,1) RVAL
    1 FORMAT(G20.7)
C
C   Find the first nonblank character.
C
      DO 10 I= 1,20
         IF (TEMP(I:I).NE.' ') GOTO 20
   10    CONTINUE
      I= 21
C
C   Copy into STRING.
C
   20 CONTINUE
      LSTRIN= 20-I+1
      IF (LSTRIN.EQ.0) THEN
         STRING= ' '
      ELSE IF (LSTRIN.GT.LEN(STRING)) THEN
         STRING= OVERFL
      ELSE
         STRING= TEMP(I:20)
      ENDIF
      RETURN
      END
      SUBROUTINE REAQAR(NEXT,IQ,RVAL,STATUS)
C
C   Convert this or the next qualifier argument to a real number.
C
C      STATUS=1 - Conversion error.
C      STATUS=2 - No more arguments.
C
      PARAMETER (MQUAL=20,MQARG=40)
      COMMON /PARQAR/ NQARG(2,MQUAL),IXQUAL,IXQARG,IFQARG
      COMMON /PARQAC/ QARG(MQARG)
      CHARACTER*256 QARG
      INTEGER STATUS
      LOGICAL NEXT
C
      STATUS= 0
C
C   Make sure we're pointing at a qualifier.
C
      IF (IQ.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier.',
     C               1)
         STOP 'REAQAR'
      ENDIF
C
C   If this is a different qualifier, reset the argument pointer and the
C   qualifier pointer.
C
      IF (IQ.NE.IXQUAL) THEN
         IXQARG= 0
         IXQUAL= IQ
      ENDIF
      IF (NEXT) THEN
         IF (IXQARG.GE.NQARG(2,IQ)) THEN
            STATUS= 2
            RETURN
         ENDIF
         IXQARG= IXQARG+1
      ENDIF
C
C   Make sure we're pointing at an argument.
C
      IF (IXQARG.EQ.0) THEN
         CALL ERRMSG('Programming error - not pointing at a qualifier'
     C    //' argument.',1)
         STOP 'REAQAR'
      ENDIF
C
C   Do the conversion on a trimmed version of the argument.
C
      I= NQARG(1,IQ)+IXQARG-1
      CALL PTRIM(QARG(I),LQARG)
      CALL CVTREA(QARG(I)(1:LQARG),RVAL,ISTAT)
      IF (ISTAT.NE.0) THEN
C
C   Error in conversion.
C
         CALL ERRMSG('Invalid real number '//QARG(I)(1:LQARG),1)
         STATUS= 1
      ENDIF
      RETURN
      END
      SUBROUTINE RSTINP
C
C   Reset the input file stack and set echo.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
C
      CHARACTER*80 STDDEV
C
C   Close and free all old unit numbers, and print a message.
C
      DO 10 I= NINSTK,1,-1
         IF (INPSTK(I).NE.0) THEN
            CLOSE(UNIT=INPSTK(I))
            CALL FRELUN(INPSTK(I))
         ENDIF
   10    CONTINUE
C
      IF (NINSTK.GT.0) THEN
         CALL MSG('Input device reset to '//STDDEV('INPUT'),1)
      ENDIF
C
      NINSTK= 0
      IOINP = 0
      CALL SETECH
      RETURN
      END
      SUBROUTINE RSTOUT
C
C   Reset the output file stack.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      PARAMETER (MSTACK=20)
      COMMON /PARIOS/ NINSTK,INPSTK(MSTACK),NOUSTK,OUTSTK(MSTACK)
      INTEGER INPSTK,OUTSTK
C
      CHARACTER*80 STDDEV
C
C   Close and free all old unit numbers, and print a message.
C
      DO 10 I= NOUSTK,1,-1
         IF (OUTSTK(I).NE.0) THEN
            CLOSE(UNIT=OUTSTK(I))
            CALL FRELUN(OUTSTK(I))
         ENDIF
   10    CONTINUE
C
      IF (NOUSTK.GT.0) THEN
         CALL MSG('Output device reset to '//STDDEV('OUTPUT'),1)
      ENDIF
C
      NOUSTK= 0
      IOOUT = 0
      CALL SETECH
      RETURN
      END
      SUBROUTINE RSTPRM
C
C   Reset the parser prompt string stack.
C
      COMMON /PARPRM/ LPRMCH
      COMMON /PARPRC/ PRMCHA
      CHARACTER*20 PRMCHA
      PARAMETER (MSTACK=20)
      COMMON /PARPST/ NPRSTK,LPRSTK(MSTACK)
      COMMON /PARPSC/ PRMSTK(MSTACK)
      CHARACTER*20 PRMSTK
C
      NPRSTK= 0
      PRMCHA= '?'
      LPRMCH= 1
      RETURN
      END
      SUBROUTINE SETECH
C
C   Determine whether the current input and output devices are batch or
C   interactive.  If either are batch, we'll want to echo input lines to the
C   output device.  Initialize prompting to be ON.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
      EXTERNAL BMODE
C
      CHARACTER*80 INPDEV,OUTDEV,STDDEV
      LOGICAL INTERA
C
C   Get the device names.
C
      IF (IOINP.NE.0) THEN
         INQUIRE(UNIT=IOINP,NAME=INPDEV)
      ELSE
         INPDEV= STDDEV('INPUT')
      ENDIF
      IF (IOOUT.NE.0) THEN
         INQUIRE(UNIT=IOOUT,NAME=OUTDEV)
      ELSE
         OUTDEV= STDDEV('OUTPUT')
      ENDIF
C
C   See if the devices are interactive or not.
C
      CALL PTRIM(INPDEV,LINPDE)
      CALL PTRIM(OUTDEV,LOUTDE)
      ECHO  =      .NOT.INTERA(INPDEV(1:LINPDE))
     C        .OR. .NOT.INTERA(OUTDEV(1:LOUTDE))
C
C   Set prompting ON.
C
      PRMPT = .TRUE.
C
      RETURN
      END
      SUBROUTINE SETMOD
C
C   Determine whether we are in batch or interactive mode.
C
      COMMON /MODE/ BATCH,ECHO,PRMPT
      LOGICAL BATCH,ECHO,PRMPT
      EXTERNAL BMODE
C
      LOGICAL INTERA
      CHARACTER*80 STDDEV
      EXTERNAL INTERA,STDDEV
C
C   See if the standard input device is interactive or not.
C
      BATCH= .NOT.INTERA(STDDEV('INPUT'))
      RETURN
      END
      SUBROUTINE PTRIM(STRING,LSTRIN)
C
C   Return the length of STRING after trailing blanks, nulls, and tabs have 
C   been removed.
C
      CHARACTER*(*) STRING
C
      CHARACTER NULL,TAB
C
C   Initialize the null and tab characters.
C
      NULL= CHAR(0)
      TAB = CHAR(9)
C
C   Loop backwards through the character string and find the last nonblank,
C   nonnull character.
C
      LSTRIN= LEN(STRING)
      DO 10 L= LSTRIN,1,-1
         IF (STRING(L:L).NE.' ' .AND. STRING(L:L).NE.NULL
     C                          .AND. STRING(L:L).NE.TAB) THEN
            LSTRIN= L
            GOTO 20
         ENDIF
   10    CONTINUE
C
C   ALL blank or null or tabs!
C
      LSTRIN= 0
C
   20 CONTINUE
      RETURN
      END
      SUBROUTINE TRNLIN(LINE,STATUS)
C
C   Look through the character string LINE, translating the symbols enclosed in
C   primes.
C
C      STATUS=1 - Error.
C
      CHARACTER*(*) LINE
      INTEGER STATUS
C
      CHARACTER*255 TRANS
      CHARACTER*80 TEXT
C
      STATUS= 0
C
C   Get the length of LINE.
C
      CALL PTRIM(LINE,LLINE)
      MAXLEN= LEN(LINE)
C
C   Look for the start of a symbol.
C
      IE= 0
  100    CONTINUE
         DO 110 I= IE+1,LLINE
C
C   If it's a prime, this is the start of a symbol.
C
            IF (LINE(I:I).EQ.'''') THEN
               IS= I+1
               GOTO 200
            ENDIF
  110       CONTINUE
C      
C   No more symbols.
C
         GOTO 400
C
C   Find the end of the symbol.
C
  200    CONTINUE
         DO 210 I= IS+1,LLINE
            IF (LINE(I:I).EQ.'''') THEN
               IE= I-1
               GOTO 300
            ENDIF
  210       CONTINUE
         IE= LLINE
C
C   Translate the symbol.
C
  300    CONTINUE
C
C   Check for a zero-length symbol first.  Can't pass this to TRNSYM.
C
         IF (IS.GT.IE) THEN
            CALL ERRMSG('No translation for zero-length symbol ''''.',1)
            STATUS= 1
            IE= MAX(IE+1,LLINE)
            GOTO 100
         ELSE
            CALL TRNSYM(LINE(IS:IE),TRANS,LTRANS,ISTAT)
C
C   If there is no translation, print an error message and set the STATUS code,
C   but continue translation.
C
            IF (ISTAT.EQ.1) THEN
               IF (LTRANS.EQ.0) THEN
                  WRITE(TEXT,301) LINE(IS-1:IE)
  301             FORMAT('No translation for symbol ',A,'''')
                  CALL ERRMSG(TEXT,1)
               ENDIF
               STATUS= 1
               IE= MAX(IE+1,LLINE)
               GOTO 100
            ENDIF
         ENDIF
C
C   Adjust IS and IE to include primes, check the resulting LINE length.
C
         IS= IS-1
         IE= MIN(IE+1,LLINE)
         NEWLEN= LLINE-(IE-IS+1)+LTRANS
         IF (NEWLEN.GT.MAXLEN) THEN
            CALL ERRMSG('Symbol translation exceeds internal limit on '
     C       //'string length.',1)
            STATUS= 1
            GOTO 400
         ENDIF
C
C   Expand or contract LINE to include the translation.
C
         CALL CCOPY(LINE,IE+1,LLINE-IE,IS+LTRANS,MAXLEN-(IS+LTRANS)+1)
         LINE(IS:IS-1+LTRANS)= TRANS(1:LTRANS)
C
C   Update our information and go back for another.
C
         IE= IS-1+LTRANS
         LLINE= NEWLEN
         GOTO 100
C
  400 CONTINUE
      RETURN
      END
      SUBROUTINE UPCASE(STRING)
C
C   Convert this character string to upper case.
C
      CHARACTER*(*) STRING
      CHARACTER*26 LOWER,UPPER
C
      DATA LOWER/'abcdefghijklmnopqrstuvwxyz'/
     C     UPPER/'ABCDEFGHIJKLMNOPQRSTUVWXYZ'/
C
C   Don't worry about the trailing blanks -
C
      CALL PTRIM(STRING,LSTRIN)
C
C   Look for lower case letters and substitute upper case ones.
C
      DO 10 I= 1,LSTRIN
         LETTER= INDEX(LOWER,STRING(I:I))
         IF (LETTER.NE.0) STRING(I:I)= UPPER(LETTER:LETTER)
C
C
   10    CONTINUE
      RETURN
      END
      SUBROUTINE WRLIN(LINE)
C
C   Write a line of text to the current output device.
C
      COMMON /PARIO/ IOINP,IOOUT,IOJOU
C
      CHARACTER*(*) LINE
C
      PARAMETER (NCOLS=255)
      CHARACTER*(NCOLS) TEXT
      LOGICAL NOCR,FIRST,LAST
C
C   Use a "$" in column 1 to signal for no carriage return at the end of the
C   line.  The format used to implement this is machine-dependent, though most
C   compilers accept (' Text',$).
C
      IF (LINE(1:1).EQ.'$') THEN
         NOCR= .TRUE.
      ELSE
         NOCR= .FALSE.
      ENDIF
C
C   Break it up into multiple lines if we have to.  Assume trailing blanks have
C   already been trimmed and that we want to print everything.  I'm going to
C   keep it under NCOLS columns, including carriage control.
C
      LLINE= LEN(LINE)
      FIRST= .TRUE.
      LAST = .FALSE.
      IS= 1
C
   10    CONTINUE
         LLEFT= LLINE-IS+1
         IF (LLEFT.GT.0) THEN
C
C   For the first (sub)line, use the supplied carriage control (except if it's
C   a "$", in which case blank it out).
C
            IF (FIRST) THEN
               TEXT= LINE(IS:)
               LTEXT= MIN(LLEFT,NCOLS)
               IF (NOCR) TEXT(1:1)= ' '
               IF (LLEFT.LE.NCOLS) LAST= .TRUE.
               IS= IS+NCOLS
               FIRST= .FALSE.
C
C   For later sublines, use a blank for carriage control.
C
            ELSE
               TEXT= ' '//LINE(IS:)
               LTEXT= 1+MIN(LLEFT,NCOLS-1)
               IF (LLEFT.LE.NCOLS-1) LAST= .TRUE.
               IS= IS+NCOLS-1
            ENDIF
C
C   For the last subline we need to check NOCR.
C
            IF (LAST.AND.NOCR) THEN
               IF (IOOUT.EQ.0) THEN
                  WRITE(    *,11) TEXT(1:LTEXT)
               ELSE
                  WRITE(IOOUT,11) TEXT(1:LTEXT)
               ENDIF
C
C   The following line is CERTAINLY FORTRAN 77 standard, though the one after
C   works on almost all compilers.
C  11          FORMAT(A)
   11          FORMAT(A,$)
C
C   Otherwise just output the text as is.
C
            ELSE
               IF (IOOUT.EQ.0) THEN
                  WRITE(    *,12) TEXT(1:LTEXT)
               ELSE
                  WRITE(IOOUT,12) TEXT(1:LTEXT)
               ENDIF
   12          FORMAT(A)
            ENDIF
            IF (.NOT.LAST) GOTO 10
         ENDIF
C
      RETURN
      END
      SUBROUTINE WRNMSG(TEXT,NLINES)
C
C   Write a warning message to the current output device.
C
      CHARACTER*(*) TEXT(*)
C
      CHARACTER*132 TEMP
C
C   Trim it and write it out.
C
      IF (NLINES.GT.0) THEN
         TEMP= '  Warning:  '//TEXT(1)
         CALL PTRIM(TEMP,LTEMP)
         CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
         DO 10 I= 2,NLINES
            TEMP= '            '//TEXT(I)
            CALL PTRIM(TEMP,LTEMP)
            CALL WRLIN(TEMP(1:MAX(LTEMP,1)))
   10       CONTINUE
      ENDIF
      RETURN
      END
